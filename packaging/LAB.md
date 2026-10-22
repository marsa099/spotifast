# Fastpotify Lab (Linux)

An opt-in, pinned edition of the Vim-navigation fork. The original package is
unchanged. Lab has a purple launcher icon and the display name **Fastpotify Lab**.

For a reproducible checkpoint build, from a checkout of this fork:

```sh
nix build --impure --file packaging/lab.nix
nix profile install --impure --file packaging/lab.nix
```

Launch **Fastpotify Lab** from the application menu, or run `fastpotify-lab`.
Installation does not launch the application or rebuild NixOS. With a fresh
profile, enable **Vim keys** under **Settings → Keyboard** to test navigation.

## Fast local development (recommended for UI iteration)

Build once, then install the small permanent launcher from your checkout:

```sh
bash packaging/update-lab-dev.sh "$PWD"
nix profile add --priority 0 --impure --file packaging/lab-dev.nix \
  --argstr sourceRoot "$PWD" launcher
```

Set `sourceRoot` to your long-lived checkout, not a disposable worktree. Once
installed, update from that checkout or explicitly choose a task worktree:

```sh
fastpotify-lab-update
fastpotify-lab-update /path/to/task-worktree
```

Restart Lab after a successful update. Updates do not launch or close windows,
change settings, or copy credentials again. The original app remains separate.

Current upstream is named **Spotifast**. Development builds use
`lab-dev-identity.nix` to keep the existing `fastpotify-lab` launcher, window ID
and XDG profile paths, with a separate `fastpotify_lab` MPRIS identity. The
legacy pinned checkpoint still uses its original `lab.nix` identity patches.
After updating from an older checkout, reinstall the small development launcher
using the command above before using `fastpotify-lab-update`.

Nix prepares an isolated source snapshot with these Lab identity patches. Tracked working-tree edits are included; newly added
source files must be staged so the Git flake includes them. The checkout itself
is never patched or built into. Cargo uses a stable private source directory and
persistent target cache, with unchanged file timestamps preserved. Library tests
and the application build share the dev profile and optimized dependencies.
MilkDrop remains enabled; demo support is included for headless screenshots.

The first build warms the cache. Later edits reuse it instead of performing two
clean optimized package builds. Changes to dependencies or the toolchain may
still need a longer rebuild. This fast path does not replace full CI or checkpoint
validation when preparing a stable version.

Build state lives under `$XDG_CACHE_HOME/fastpotify-lab-dev` (normally
`~/.cache/fastpotify-lab-dev`). Do not edit its managed source mirror. Installed
builds and their per-generation Nix environment GC roots live under
`$XDG_DATA_HOME/fastpotify-lab-dev` (normally `~/.local/share/fastpotify-lab-dev`),
so clearing compilation caches does not remove the installed app or its libraries.
Only checked builds replace the `current` symlink, atomically; `previous` retains
the preceding successful generation. Failed updates keep the current build.
Each generation's `build-info` records the original checkout path, the exact
Nix source snapshot and the UTC build time. Updating repository source alone
does not rebuild or replace an installed generation.

Keep the lower-priority checkpoint package installed as a fallback. Removing the
`fastpotify-lab-dev` profile entry restores that launcher without deleting Lab's
settings or sign-in. Find exact entry names with `nix profile list`.

Updater safety checks (no Rust compilation or real account access):

```sh
bash packaging/test-update-lab-dev.sh
```

## Side-by-side isolation

- Config, state and cache use `fastpotify-lab` instead of `fastpotify` under the
  normal XDG directories. Lab does not automatically copy an existing profile.
- Credential storage retains the existing Secret Service service name but uses
  a different profile key, derived from the Lab state path. Copying settings
  alone does **not** copy a current version's protected sign-in credentials.
- Window identity, single-instance channel, MPRIS name, tray identity and
  desktop filename are separate. Lab does not register Spotify URI associations.
- The default Spotify Connect device name is **Fastpotify Lab**. Keep it distinct
  when importing settings: the Connect device ID is derived from that name.

Both clients still use the same Spotify account when credentials are copied.
Account-side edits affect both clients, and Spotify playback restrictions still
apply. Local profile isolation is not account isolation.

## Updating or removing Lab

The source revision is pinned in `lab.nix`. To test another revision, update that
pin, rebuild and test the package, then reinstall it. Packaging substitutions use
`--replace-fail` so source changes cannot silently disable isolation.

Use `nix profile list` to find the installed Lab entry and
`nix profile remove <entry-name>` to remove it. This leaves its settings and
credentials intact and never removes the original app.

The package build runs the inherited Rust tests. No visible application is
started during the build. Only Linux packaging is supported by this recipe.
