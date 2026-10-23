# Source preparation and permanent launchers. Rust builds stay incremental.
{
  system ? builtins.currentSystem,
  fork ? builtins.getFlake ("git+file://" + toString ../.),
  sourceRoot ? toString ../.,
}:
let
  pkgs = import fork.inputs.nixpkgs { inherit system; };
  labIdentity = import ./lab-dev-identity.nix;
  updater = import ./fork-update/package.nix { inherit pkgs; };
  source = pkgs.runCommand "spotifast-lab-dev-source" { } ''
    mkdir -p "$out"
    cp -R ${fork.outPath}/. "$out/"
    chmod -R u+w "$out"
    cd "$out"
    ${labIdentity}
  '';
  launch = pkgs.writeShellScriptBin "spotifast-lab" ''
    export SPOTIFAST_FORK_UPDATER=${updater}/bin/spotifast-fork-updater
    data="''${XDG_DATA_HOME:-$HOME/.local/share}/spotifast-lab-dev"
    generation=$(${pkgs.coreutils}/bin/readlink -e "$data/current") || {
      echo 'Lab has no development build yet. Run spotifast-lab-update first.' >&2
      exit 1
    }
    exec "$generation/run" "$@"
  '';
  update = pkgs.writeShellScriptBin "spotifast-lab-update" ''
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
    name = "spotifast-lab";
    desktopName = "Spotifast Lab";
    genericName = "Spotify fork testing";
    comment = "Incremental development edition with an isolated profile";
    exec = "spotifast-lab";
    icon = "spotifast-lab";
    startupWMClass = "spotifast-lab";
    categories = [ "AudioVideo" "Audio" "Player" ];
  };
in
assert pkgs.stdenv.hostPlatform.isLinux;
{
  inherit source;
  launcher = pkgs.symlinkJoin {
    name = "spotifast-lab-dev";
    paths = [ launch update desktop ];
    postBuild = ''
      install -Dm644 ${source}/packaging/icons/spotifast.svg \
        "$out/share/icons/hicolor/scalable/apps/spotifast-lab.svg"
    '';
    meta = {
      description = "Spotifast Lab launchers for cached local development builds";
      mainProgram = "spotifast-lab";
      platforms = pkgs.lib.platforms.linux;
    };
  };
}
