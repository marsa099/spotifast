# Reproducible isolated Linux package from this checkout and its flake lock.
# Build: nix build --impure --file packaging/lab.nix
{
  system ? builtins.currentSystem,
  fork ? builtins.getFlake ("git+file://" + toString ../.),
}:
let
  pkgs = import fork.inputs.nixpkgs { inherit system; };
  base = fork.packages.${system}.spotifast;
  labIdentity = import ./lab-dev-identity.nix;
  updater = import ./fork-update/package.nix { inherit pkgs; };
  desktop = pkgs.makeDesktopItem {
    name = "spotifast-lab";
    desktopName = "Spotifast Lab";
    genericName = "Spotify fork testing";
    comment = "Vim-enabled test edition with a separate profile";
    exec = "spotifast-lab";
    icon = "spotifast-lab";
    startupWMClass = "spotifast-lab";
    categories = [ "AudioVideo" "Audio" "Player" ];
    # Upstream remains the Spotify URI handler.
  };
in
assert pkgs.stdenv.hostPlatform.isLinux;
base.overrideAttrs (old: {
  pname = "spotifast-lab";
  postPatch = (old.postPatch or "") + labIdentity;
  postInstall = ''
    mv "$out/bin/spotifast" "$out/bin/spotifast-lab"
    cp -R ${updater}/bin/. "$out/bin/"
    cp -R ${updater}/share/. "$out/share/"
    install -Dm644 ${desktop}/share/applications/spotifast-lab.desktop \
      "$out/share/applications/spotifast-lab.desktop"
    install -Dm644 packaging/icons/spotifast.svg \
      "$out/share/icons/hicolor/scalable/apps/spotifast-lab.svg"
  '';
  postFixup = builtins.replaceStrings
    [ "$out/bin/spotifast" ] [ "$out/bin/spotifast-lab" ] (old.postFixup or "");
  meta = old.meta // {
    description = "Spotifast Lab: isolated Vim-navigation test edition";
    mainProgram = "spotifast-lab";
    platforms = pkgs.lib.platforms.linux;
  };
})
