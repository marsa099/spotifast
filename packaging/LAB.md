# Spotifast Lab (Linux)

This fork adds opt-in Vim navigation and Alt+H/L seeking to upstream Spotifast.
Lab uses the **Spotifast Lab** name, purple icon, `spotifast-lab` window ID,
`spotifast_lab` MPRIS identity and a separate profile. It does not register
Spotify URI associations or replace the normal `spotifast` package.

## Incremental development

From the long-lived `~/repos/spotifast` checkout:

```sh
bash packaging/update-lab-dev.sh "$PWD"
nix profile add --priority 0 --impure --file packaging/lab-dev.nix \
  --argstr sourceRoot "$PWD" launcher
```

The installed commands are:

```sh
spotifast-lab
spotifast-lab-update
spotifast-lab-update /path/to/task-worktree
```

The updater tests and builds an isolated source snapshot. It never compiles in
or patches the checkout. New files must be staged for the Git flake snapshot.
Dependencies and unchanged source timestamps are cached across builds.
MilkDrop and deterministic demo support are enabled.

Builds never open, close or restart application windows. Only a checked build
atomically replaces `current`; `previous` retains the preceding generation.
A running process keeps using its original binary. Restart it yourself when
you want to use the selected new generation.

Compilation cache lives under `$XDG_CACHE_HOME/spotifast-lab-dev`. Its `target/`
directory is disposable; removing it only makes future builds slower.
Installed generations and their runtime environment GC roots live under
`$XDG_DATA_HOME/spotifast-lab-dev`, not in the compilation cache.
Each `build-info` records the checkout path, immutable source snapshot and UTC
build time. The checkout path alone is not proof of a clean Git revision.

Test updater rollback and isolation without compiling Rust or using an account:

```sh
bash packaging/test-update-lab-dev.sh
```

## Existing Lab installations

The previous Lab name was **Fastpotify Lab**. During a live rename, keep its
existing config, state, application data and generations in place. New-named
XDG paths can be symlinks to the corresponding `fastpotify-lab` and
`fastpotify-lab-dev` directories. Do not move or delete files used by a running
process, and never overwrite an existing new-named directory.

The state directory's non-secret `credential-profile` records the SHA-256
profile ID of the original state path, unless it already has a recorded ID.
Lab deliberately retains its existing keyring service namespace in
`lab-dev-identity.nix`; its profile remains separate from the normal app.
Both are needed to preserve protected sign-in. Settings alone do not contain
those credentials. Do not copy, print or export keyring secrets.

The rename does not rewrite the saved Spotify Connect device name. Existing
custom names and device identity are preserved; new profiles default to
**Spotifast Lab**.

A Super+N wrapper can recognize both old and new window IDs while the old
window is still running, focusing it rather than starting a duplicate.
After that window closes normally, the next launch uses the selected new build.

## Reproducible package

`lab.nix` builds the same identity transform as development builds, from this
checkout and its locked dependencies:

```sh
nix build --impure --file packaging/lab.nix
```

Use a clean, committed checkout to identify a reproducible revision.
Substitutions use `--replace-fail` so upstream identity changes cannot silently
break isolation. The package runs inherited Rust tests without starting a GUI.
Only Linux Lab packaging is supported.

## Removal

Use `nix profile list` to find the exact launcher/package entry before removing
it with `nix profile remove <entry-name>`. Settings, credentials, installed
generations and the normal Spotifast application are left alone.
