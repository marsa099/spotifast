# Host companion only. Application builds remain click-triggered containers.
{ pkgs }:
pkgs.runCommand "spotifast-fork-updater" { } ''
  mkdir -p "$out/bin" "$out/share/spotifast/fork-update"
  for asset in worker.py build.py pi.ts Dockerfile contract.md lab-identity.json; do
    install -m644 ${./.}/"$asset" "$out/share/spotifast/fork-update/$asset"
  done
  substitute ${./worker.py} "$out/bin/spotifast-fork-updater" \
    --replace-fail '#!/usr/bin/env python3' '#!${pkgs.python3}/bin/python3'
  chmod 755 "$out/bin/spotifast-fork-updater"
''
