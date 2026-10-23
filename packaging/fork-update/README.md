# Click-triggered Lab fork updater

**Status: source implementation; real container/AppImage integration is not yet verified.**
Nothing here schedules updates, rebuilds the system, changes the running checkout,
pushes Git refs, or automatically installs a finished candidate.

## Flow

1. Click the existing update button. Its label is unchanged. A private tmux worker
   fetches the published fork main and the selected upstream release tag into
   separate Git storage. Ordinary release discovery can still run beforehand;
   integration, repair and builds cannot.
2. Integrate without rewriting history. Straightforward content conflicts can be
   repaired by host Pi using only container-backed shell tools. An existing active
   worker is reused on repeated clicks. Questions appear in the same interactive
   Pi terminal, reachable through **Needs your input** / **Open update terminal**.
3. Check formatting, companion tests, all-feature Rust tests and Clippy, then build
   and version-check a Lab AppImage in a disposable source copy. No stock upstream
   executable is downloaded as a substitute. The normal signed upstream installer
   is not weakened or used as a fallback.
4. Click **Restart to update** only when ready. A SHA-256-checked generation is
   staged; `current` switches atomically and `previous` retains the old generation.
   The supervisor waits at most 60 seconds for the actual app PID to exit and never
   kills it. A supervisor spawn failure restores both selectors.

The AppImage carries its companion assets. The initial trusted orchestration
assets are snapshotted at click time and used for this integration/build; candidate
source cannot silently replace the running controller or its build mandate.

## Hard limits

| Operation | Maximum |
| --- | --- |
| App control request / helper command / container cleanup | 90 s / 60 s / 15 s |
| Each Git operation | 5 min |
| Candidate build | 45 min total; 10 min without output; each build tool 20 min |
| Pi repair / individual shell tool / ordinary terminal input wait | 30 min / at most 15 min plus 15 s transport allowance / 15 min |
| Entire worker / UI monitor | 2 h / 2 h 1 min |

Version probes and formatting have shorter limits (30 s and 60 s). There are at
most three explicitly retried attempts per failing stage, and at most two automatic
repair rounds for ordinary build failures. A build time/memory limit stops the
update rather than starting another automatic repair. Timed-out build containers
are force-removed with a separate deadline. A stopped worker publishes `failed`;
it never publishes `ready` or installs a candidate on that path.

These limits apply to updater work, not to the music app's normal lifetime or a
terminal window the user keeps open. OS-level uninterruptible I/O cannot be given
a reliable userspace termination guarantee.

## Setup

Linux x86_64 and aarch64; Python 3.11+, GNU coreutils (`timeout`), Git, tmux, Pi,
Docker or Podman, and one supported graphical terminal (Kitty, foot, Alacritty,
xterm). NixOS additionally needs an already available `appimage-run`; Arch does
not need Nix. The worker does not install prerequisites or rebuild NixOS.

Pi reuses its ordinary host authentication and default built-in provider/model.
Global extensions, MCP servers, context files, skills and prompt templates are
not loaded. Custom provider extensions are not automatically loaded either; use
a built-in provider for this restricted session. No provider tokens are passed
to containers. Do not paste secrets into repair conversations or logs.

The trusted image must be built once, explicitly. The worker will pause and show
the exact command when missing; it never starts this environment build itself:

```sh
timeout --kill-after=10s 45m docker build -t spotifast-updater:rust-1.98-v1 -f packaging/fork-update/Dockerfile packaging/fork-update
```

Run from this checkout; substitute `podman` if appropriate. Creating the image
requires downloads and is separate from the resource-limited candidate build.
Candidate/repair containers have an eight-CPU quota, 8 GiB memory limit, no extra
swap allowance, no added capabilities, and a 512-process limit. The worker checks
for at least 12 GiB free disk space; this is a minimum, not a disk quota.

Nix Lab packages include the companion; the development launcher points at it
explicitly. A missing companion is an error, not permission to install vanilla
upstream binaries. Neither current source edits nor these instructions modify an
already installed Lab package.

## Data, isolation and rollback

Private status, logs, source and caches live under the app state directory's
`fork-updater/`. Logs and Pi sessions may contain sensitive conversation text;
keep them private. Build/repair containers mount only a disposable source tree,
a private build cache, and read-only trusted tools. Repair has no network and no
Git metadata. Neither mode mounts home, credentials, the Docker socket, desktop
services or SSH agents.

Automatic copy-back is restricted to navigation/UI Rust files, app/model/settings
hooks, and Markdown guide files. Embedded tests and other paths require terminal
review with the exact approval phrase. This is not permission to weaken tests,
change credentials, or broaden the sandbox. Independent checks still run.

Lab identity preparation uses `lab-identity.json`, shared with the existing Nix
Lab transform. Existing settings/profile paths and legacy keyring identity are
preserved; no user profile or credentials are copied into a build. A different
OS installation may still require signing in to its own keyring.

Generations live under `${XDG_DATA_HOME:-$HOME/.local/share}/spotifast-lab-dev`.
No generation is automatically deleted. To roll back, close the app normally,
inspect `current` and `previous`, then atomically replace only the `current`
symlink with the verified previous target. Do not replace an ordinary file or
remove generation directories. A portable generation can be run directly via
`current/run`, without the old Nix launcher on Arch. Install Python and recreate
home-path symlinks if migrating directories or usernames; copy the real data,
not only dangling legacy aliases.

## Lightweight checks (no real updater/build)

```sh
timeout --kill-after=5s 30s python3 -B packaging/fork-update/test_worker.py
timeout --kill-after=5s 30s cargo fmt --all --check
```

The Python suite uses fake tools and temporary fixture repositories. It covers
click-only startup, singleton locking, reconnect, sandbox configuration, protected
edits, silent/total deadlines, bounded retries, source-isolated conflict repair,
candidate integrity, selector restoration and retained rollback generations.
A real image build, Rust compilation, graphical restart and installed-app update
must be verified separately before calling the feature complete.
