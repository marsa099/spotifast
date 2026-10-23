#!/usr/bin/env python3
"""Trusted container-side checks and portable AppImage packaging."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import tomllib

TOOLS = Path(__file__).resolve().parent


def run(arguments, *, capture=False, cwd=None, env=None, timeout=1200):
    return subprocess.run(["timeout", "--kill-after=5s", str(timeout),
                           *[str(value) for value in arguments]], cwd=cwd, env=env,
                          check=True, text=True, capture_output=capture, timeout=timeout + 10)


def apply_identity(root: Path, recipe: Path) -> None:
    rules = json.loads(recipe.read_text())
    prepared = []
    for rule in rules:
        relative = Path(rule["path"])
        if relative.is_absolute() or ".." in relative.parts:
            raise RuntimeError("Identity recipe escapes the source tree")
        path = root / relative
        if path.is_symlink():
            raise RuntimeError("Identity recipe cannot patch source symlinks")
        text = path.read_text()
        for old, new in rule["replacements"]:
            if not old or old not in text:
                raise RuntimeError(f"Lab identity changed upstream: {relative}: {old!r}")
            text = text.replace(old, new)
        prepared.append((path, text))
    # Validate every replacement before writing any source file.
    for path, text in prepared:
        path.write_text(text)


def package_configuration(root: Path, version: str, target: str, archive: Path) -> dict:
    original = run(["ruby", "-rjson", "-ryaml", "-e",
                    "puts JSON.generate(YAML.safe_load_file(ARGV.fetch(0), aliases: false))",
                    root / "native-packages.yaml"], capture=True)
    config = json.loads(original.stdout)
    selected = config["targets"][target]
    selected["input"] = {"local": str(archive)}
    # A locally integrated fork must never advertise upstream's binary channel
    # to external AppImageUpdate tools. This updater owns the fork's checks.
    selected.setdefault("appimage", {}).pop("update_information", None)
    config["targets"] = {target: selected}
    payload = f"@PAYLOAD@/*-v@VERSION@-{selected['compiler_target']}"
    contents = config["nfpm"].setdefault("contents", [])
    for item in contents:
        if item["src"].startswith("contrib/"):
            item["src"] = payload + "/" + item["src"]
    contents.append({"src": payload + "/spotifast-fork-updater",
                     "dst": "/usr/bin/", "file_info": {"mode": 493}})
    for name in ("worker.py", "build.py", "pi.ts", "Dockerfile", "contract.md", "lab-identity.json"):
        contents.append({"src": payload + "/packaging/fork-update/" + name,
                         "dst": "/usr/share/spotifast/fork-update/",
                         "file_info": {"mode": 420}})
    config["nfpm"]["homepage"] = "https://github.com/marsa099/spotifast"
    return config


def build(version: str, root: Path, output: Path) -> None:
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z][0-9A-Za-z.-]*)?", version):
        raise RuntimeError("Invalid release version")
    manifest = tomllib.loads((root / "Cargo.toml").read_text())
    if manifest["package"]["version"] != version:
        raise RuntimeError("Merged source version does not match the selected upstream release")
    architecture = platform.machine()
    if architecture == "x86_64":
        target, triple = "appimage-amd64", "x86_64-unknown-linux-gnu"
    elif architecture in {"aarch64", "arm64"}:
        target, triple = "appimage-arm64", "aarch64-unknown-linux-gnu"
    else:
        raise RuntimeError("Portable updates currently support Linux x86_64 and aarch64")
    Path.home().mkdir(parents=True, exist_ok=True)
    run(["python3", "-B", "packaging/fork-update/test_worker.py"], cwd=root, timeout=60)
    run(["cargo", "fmt", "--all", "--check"], cwd=root, timeout=60)
    apply_identity(root, TOOLS / "lab-identity.json")
    # Identity preparation is a generated build-tree transform; format that
    # generated copy, never the user's checkout or the integration Git source.
    run(["cargo", "fmt", "--all"], cwd=root, timeout=60)
    run(["cargo", "clippy", "--locked", "--all-targets", "--all-features", "--", "-D", "warnings"], cwd=root)
    run(["cargo", "test", "--locked", "--all-targets", "--all-features"], cwd=root)
    run(["cargo", "build", "--locked", "--release", "--target", triple], cwd=root)
    binary = Path(os.environ["CARGO_TARGET_DIR"]) / triple / "release" / "spotifast"
    actual = run([binary, "--version"], capture=True, timeout=30).stdout.strip()
    if actual != "spotifast " + version:
        raise RuntimeError("Built program does not report the expected version: " + actual)

    payload = output / f"spotifast-v{version}-{triple}"
    payload.mkdir(parents=True, exist_ok=False)
    shutil.copy2(binary, payload / "spotifast")
    for name in ["README.md", "LICENSE", "packaging/spotifast-portable.txt",
                 "packaging/applications/spotifast.desktop", "packaging/icons/spotifast.svg",
                 "contrib/omarchy/spotifast.json.tpl", "contrib/omarchy/spotifast-theme"]:
        destination = payload / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(root / name, destination)
    desktop = payload / "packaging/applications/spotifast.desktop"
    desktop.write_text(desktop.read_text().replace("Name=Spotifast\n", "Name=Spotifast Lab\n")
                       .replace("StartupWMClass=spotifast\n", "StartupWMClass=spotifast-lab\n"))
    bundled_tools = payload / "packaging/fork-update"
    bundled_tools.mkdir(parents=True)
    for name in ["worker.py", "build.py", "pi.ts", "Dockerfile", "contract.md", "lab-identity.json"]:
        shutil.copy2(TOOLS / name, bundled_tools / name)
    shutil.copy2(TOOLS / "worker.py", payload / "spotifast-fork-updater")
    (payload / "spotifast-fork-updater").chmod(0o755)
    archive = output / (payload.name + ".tar.gz")
    with tarfile.open(archive, "w:gz") as compressed:
        compressed.add(payload, arcname=payload.name)
    package_root = output / "package"
    package_root.mkdir()
    config = package_configuration(root, version, target, archive)
    # JSON is YAML-compatible; avoid a second host/runtime YAML dependency.
    (package_root / "native-packages.yaml").write_text(json.dumps(config, indent=2))
    run(["native-packages", "build", "--version", version, "--target", target,
         "--defer-recipes", "--output", output / "packaged"], cwd=package_root)
    images = list((output / "packaged/packages" / target / "appimage").glob("*.AppImage"))
    if len(images) != 1:
        raise RuntimeError("Packaging did not produce exactly one AppImage")
    candidate = output / "update.AppImage"
    shutil.copy2(images[0], candidate)
    candidate.chmod(0o700)
    runtime_cache = output / "runtime-check"
    runtime_cache.mkdir()
    environment = os.environ.copy()
    environment.update(APPIMAGE_EXTRACT_AND_RUN="1", TMPDIR=str(runtime_cache))
    checked = run([candidate, "--version"], capture=True, env=environment, timeout=30).stdout.strip()
    if checked != "spotifast " + version:
        raise RuntimeError("Packaged AppImage does not report the expected version")
    print("Verified portable fork AppImage:", candidate, flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    arguments = parser.parse_args()
    try:
        build(arguments.version, Path("/workspace"), Path("/output"))
    except subprocess.TimeoutExpired as error:
        print(f"Build tool deadline reached: {error}", file=sys.stderr)
        sys.exit(124)
    except subprocess.CalledProcessError as error:
        if error.returncode in {124, 137}:
            print(f"Build tool stopped on a time/resource limit: {error}", file=sys.stderr)
            sys.exit(error.returncode)
        raise
