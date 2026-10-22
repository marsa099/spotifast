#!/usr/bin/env bash
# Test deployment safety without compiling Rust, using a real temporary Git repo.
set -euo pipefail
umask 077
script=$(realpath "$(dirname "$0")/update-lab-dev.sh")
tmp=$(mktemp -d /tmp/spotifast-lab-updater-test.XXXXXXXX)
mkdir -p "$tmp/bin" "$tmp/repo/packaging" "$tmp/source"
git -C "$tmp/repo" init -q
printf 'initial\n' > "$tmp/source/example.rs"
export XDG_CACHE_HOME="$tmp/cache" XDG_DATA_HOME="$tmp/data" FAKE_SOURCE="$tmp/source"
export PATH="$tmp/bin:$PATH"
cat > "$tmp/bin/nix" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
if [[ $1 == build ]]; then
    [[ ${FAIL_SOURCE:-0} == 0 ]] || exit 1
    printf '%s\n' "$FAKE_SOURCE"
else
    [[ $2 == "path:$FAKE_SOURCE" ]]
    [[ $3 == --profile && $4 == "$XDG_DATA_HOME/spotifast-lab-dev/generations/"*/environment ]]
    while [[ $1 != --command ]]; do shift; done
    shift
    exec "$@"
fi
SH
cat > "$tmp/bin/cargo" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
if [[ $1 == test ]]; then
    [[ ${FAIL_TESTS:-0} == 0 ]]
else
    [[ ${FAIL_BUILD:-0} == 0 ]] || exit 1
    mkdir -p "$CARGO_TARGET_DIR/debug"
    {
        printf '#!/bin/sh\nprintf "fake Lab version\\n"\n'
        if [[ ${BAD_IDENTITY:-0} == 0 ]]; then
            printf '# %s\n' spotifast_lab spotifast-lab 'Spotifast Lab'
        fi
    } > "$CARGO_TARGET_DIR/debug/spotifast"
    chmod 755 "$CARGO_TARGET_DIR/debug/spotifast"
fi
SH
chmod 755 "$tmp/bin/nix" "$tmp/bin/cargo"
run() { bash "$script" "$tmp/repo" > "$tmp/run.log" 2>&1; }
run
current="$XDG_DATA_HOME/spotifast-lab-dev/current"
first=$(readlink "$current")
test -x "$first/run"
test -s "$first/build-info"
grep -Fx "repo=$tmp/repo" "$first/build-info"
test "$(stat -c %a "$XDG_DATA_HOME/spotifast-lab-dev")" = 700
printf 'PASS: successful checked build is deployed with private storage\n'
for failure in FAIL_SOURCE FAIL_TESTS FAIL_BUILD BAD_IDENTITY; do
    if env "$failure=1" bash "$script" "$tmp/repo" > "$tmp/run.log" 2>&1; then
        echo "FAIL: $failure should prevent deployment" >&2
        exit 1
    fi
    test "$(readlink "$current")" = "$first"
done
printf 'PASS: source, test, build and identity failures preserve the previous build\n'
mtime=$(stat -c %y "$XDG_CACHE_HOME/spotifast-lab-dev/src/example.rs")
sleep 1
run
test "$(stat -c %y "$XDG_CACHE_HOME/spotifast-lab-dev/src/example.rs")" = "$mtime"
test "$(readlink "$XDG_DATA_HOME/spotifast-lab-dev/previous")" = "$first"
printf 'PASS: unchanged source keeps mtimes; the prior generation is retained\n'
printf 'changed\n' > "$tmp/source/example.rs"
touch -r "$XDG_CACHE_HOME/spotifast-lab-dev/src/example.rs" "$tmp/source/example.rs"
sleep 1
run
test "$(stat -c %y "$XDG_CACHE_HOME/spotifast-lab-dev/src/example.rs")" != "$mtime"
printf 'PASS: changed contents invalidate Cargo even with unchanged source timestamps\n'
latest=$(readlink "$current")
exec 8> "$XDG_CACHE_HOME/spotifast-lab-dev/update.lock"
flock -n 8
if run; then echo 'FAIL: concurrent update was allowed' >&2; exit 1; fi
flock -u 8
exec 8>&-
test "$(readlink "$current")" = "$latest"
mkdir -p "$tmp/unmanaged/spotifast-lab-dev/src"
printf 'keep\n' > "$tmp/unmanaged/spotifast-lab-dev/src/user-file"
if XDG_CACHE_HOME="$tmp/unmanaged" run; then echo 'FAIL: unmanaged source was overwritten' >&2; exit 1; fi
test -f "$tmp/unmanaged/spotifast-lab-dev/src/user-file"
test "$(readlink "$current")" = "$latest"
mkdir -p "$tmp/unmanaged-data/spotifast-lab-dev"
printf 'keep\n' > "$tmp/unmanaged-data/spotifast-lab-dev/current"
if XDG_DATA_HOME="$tmp/unmanaged-data" run; then echo 'FAIL: unmanaged data was overwritten' >&2; exit 1; fi
test "$(<"$tmp/unmanaged-data/spotifast-lab-dev/current")" = keep
printf 'PASS: concurrent updates and unmanaged source/data overwrites are refused\n'
