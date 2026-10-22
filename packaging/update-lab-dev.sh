#!/usr/bin/env bash
# Build an isolated Lab snapshot. Never compile or patch the source checkout.
set -euo pipefail
umask 077

if [[ ${1:-} == --build-inside ]]; then
    cache=${2:?}
    generation=${3:?}
    export CARGO_TARGET_DIR="$cache/target"
    export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"
    export CMAKE="$cache/cmake-libdir"
    if [[ ! -e $CMAKE ]]; then
        printf '#!%s\n' "$(command -v bash)" > "$CMAKE"
        # Arguments belong to the generated wrapper's invocation, not this build.
        # shellcheck disable=SC2016
        printf '%s\n' 'if [[ ${1:-} == --build ]]; then exec cmake "$@"; else exec cmake "$@" -DCMAKE_INSTALL_LIBDIR=lib; fi' >> "$CMAKE"
        chmod 700 "$CMAKE"
    fi
    cd "$cache/src"
    # Both commands use the dev profile (unwind), sharing optimized dependencies.
    # Demo support permits isolated screenshots; normal launches still use Spotify.
    cargo test --locked --lib --features demo
    cargo build --locked --features demo
    binary="$CARGO_TARGET_DIR/debug/spotifast"
    for marker in fastpotify_lab fastpotify-lab 'Fastpotify Lab'; do
        if ! grep -aFq "$marker" "$binary"; then
            echo 'Refusing to deploy a binary without the Lab identities.' >&2
            exit 1
        fi
    done
    "$binary" --version
    install -m755 "$binary" "$generation/fastpotify"
    # Capture the matching runtime environment, not a later shell's libraries.
    {
        printf '#!%s\n' "$(command -v bash)"
        # Preserve expansion of the launching process's environment at runtime.
        # shellcheck disable=SC2016
        printf 'export LD_LIBRARY_PATH=%q:"${LD_LIBRARY_PATH:-}"\n' "${LD_LIBRARY_PATH:-}"
        printf 'exec %q "$@"\n' "$generation/fastpotify"
    } > "$generation/run"
    chmod 755 "$generation/run"
    "$generation/run" --version
    exit 0
fi

if [[ $# -ne 1 ]]; then
    echo 'Usage: update-lab-dev.sh /path/to/fastpotify-checkout' >&2
    exit 2
fi
repo=$(git -C "$(realpath "$1")" rev-parse --show-toplevel)
cache="${XDG_CACHE_HOME:-$HOME/.cache}/fastpotify-lab-dev"
data="${XDG_DATA_HOME:-$HOME/.local/share}/fastpotify-lab-dev"
mkdir -p "$cache" "$data/generations"
exec 9> "$cache/update.lock"
flock -n 9 || { echo 'Another Lab update is already running.' >&2; exit 1; }
if [[ -L $cache/src || ( -e $cache/src && ! -f $cache/managed-source ) ]]; then
    echo 'Refusing to overwrite an unmanaged source directory.' >&2
    exit 1
fi
for link in current previous; do
    if [[ -e $data/$link && ! -L $data/$link ]]; then
        echo "Refusing to replace unmanaged Lab data: $link" >&2
        exit 1
    fi
done
trap 'echo "Update failed; the previously selected Lab build was not replaced." >&2' ERR

printf '[1/3] Preparing isolated Lab source\n'
source=$(nix build --impure --file "$repo/packaging/lab-dev.nix" source \
    --out-link "$cache/source-result" --print-out-paths)
mkdir -p "$cache/src"
touch "$cache/managed-source"
# No --times: changed source must get a new mtime so Cargo detects edits.
# Checksums leave unchanged files untouched, preserving incremental fingerprints.
rsync -r --links --checksum --perms --chmod=u+w --delete "$source/" "$cache/src/"

printf '[2/3] Testing and building with the persistent Cargo cache\n'
generation=$(mktemp -d "$data/generations/build.XXXXXXXX")
# Each generation pins its own environment outside the cache, including the
# previous build's libraries when a later update changes the Nix inputs.
nix develop "path:$source" --profile "$generation/environment" --command \
    bash "$(realpath "$0")" --build-inside "$cache" "$generation"
printf 'repo=%s\nsource=%s\nbuilt=%s\n' "$repo" "$source" "$(date -u +%FT%TZ)" > "$generation/build-info"

printf '[3/3] Selecting the tested build atomically\n'
if [[ -L $data/current ]]; then
    ln -s "$(readlink "$data/current")" "$generation/previous-link"
    mv -Tf "$generation/previous-link" "$data/previous"
fi
ln -s "$generation" "$generation/current-link"
mv -Tf "$generation/current-link" "$data/current"
printf 'Lab updated in %s seconds. Restart Fastpotify Lab when ready.\n' "$SECONDS"
