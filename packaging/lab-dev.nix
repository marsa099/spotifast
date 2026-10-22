# Tiny Nix packages only: source preparation and permanent launchers.
# Rust compilation is performed incrementally by update-lab-dev.sh.
{
  system ? builtins.currentSystem,
  fork ? builtins.getFlake (toString ../.),
  sourceRoot ? toString ../.,
}:
let
  pkgs = import fork.inputs.nixpkgs { inherit system; };
  labIdentity = import ./lab-dev-identity.nix;
  source = pkgs.runCommand "fastpotify-lab-dev-source" { } ''
    mkdir -p "$out"
    cp -R ${fork.outPath}/. "$out/"
    chmod -R u+w "$out"
    cd "$out"
    ${labIdentity}
  '';
  launch = pkgs.writeShellScriptBin "fastpotify-lab" ''
    data="''${XDG_DATA_HOME:-$HOME/.local/share}/fastpotify-lab-dev"
    generation=$(${pkgs.coreutils}/bin/readlink -e "$data/current") || {
      echo 'Lab has no development build yet. Run fastpotify-lab-update first.' >&2
      exit 1
    }
    exec "$generation/run" "$@"
  '';
  update = pkgs.writeShellScriptBin "fastpotify-lab-update" ''
    export PATH=${
      pkgs.lib.makeBinPath [
        pkgs.nix
        pkgs.git
        pkgs.rsync
        pkgs.util-linux
        pkgs.coreutils
      ]
    }:"$PATH"
    if [ "$#" -eq 0 ]; then set -- ${pkgs.lib.escapeShellArg sourceRoot}; fi
    exec ${pkgs.bash}/bin/bash ${./update-lab-dev.sh} "$@"
  '';
  desktop = pkgs.makeDesktopItem {
    name = "fastpotify-lab";
    desktopName = "Fastpotify Lab";
    genericName = "Spotify fork testing";
    comment = "Incremental development edition with an isolated profile";
    exec = "fastpotify-lab";
    icon = "fastpotify-lab";
    startupWMClass = "fastpotify-lab";
    categories = [
      "AudioVideo"
      "Audio"
      "Player"
    ];
  };
in
assert pkgs.stdenv.hostPlatform.isLinux;
{
  inherit source;
  launcher = pkgs.symlinkJoin {
    name = "fastpotify-lab-dev";
    paths = [
      launch
      update
      desktop
    ];
    postBuild = ''
      install -Dm644 ${source}/packaging/icons/spotifast.svg \
        "$out/share/icons/hicolor/scalable/apps/fastpotify-lab.svg"
    '';
    meta = {
      description = "Fastpotify Lab launchers for cached local development builds";
      mainProgram = "fastpotify-lab";
      platforms = pkgs.lib.platforms.linux;
    };
  };
}
