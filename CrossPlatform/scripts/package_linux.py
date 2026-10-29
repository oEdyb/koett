#!/usr/bin/env python3
"""Package a local Linux candidate without installing it or publishing a release."""

import argparse
import hashlib
import json
import platform
import shutil
import subprocess
import tarfile
from pathlib import Path


def digest(path):
    value = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/release/koett"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--gtk-runtime", type=Path, required=True)
    parser.add_argument("--asr-runtime", type=Path, required=True)
    parser.add_argument("--llama-runtime", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    binary = args.binary.resolve()
    if not binary.is_file():
        parser.error(f"binary not found: {binary}")
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    dirty = bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=root, text=True))
    name = f"Koett-Linux-{platform.machine()}-{revision[:8]}"
    output = args.output.resolve()
    destination = output / name
    destination.mkdir(parents=True, exist_ok=False)
    (destination / "libexec").mkdir()
    shutil.copy2(binary, destination / "libexec/koett")
    shutil.copy2(root / "packaging/linux/koett-launcher", destination / "koett")
    (destination / "koett").chmod(0o755)
    for kind, runtime in [("gtk", args.gtk_runtime), ("asr", args.asr_runtime)]:
        shutil.copytree(runtime.resolve(), destination / "runtime" / kind, symlinks=False)
    shutil.copy2(root / "packaging/linux/install.py", destination / "install.py")
    shutil.copy2(root / "packaging/linux/preflight.py", destination / "preflight.py")
    shutil.copytree(args.llama_runtime.resolve(), destination / "runtime/llama", symlinks=False)
    (destination / "runtime/bin").mkdir()
    wrapper = destination / "runtime/bin/llama-server"
    wrapper.write_text('#!/bin/sh\nROOT=$(dirname -- "$(readlink -f -- "$0")")/../llama\nexport LD_LIBRARY_PATH="$ROOT${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"\nexec "$ROOT/llama-server" "$@"\n')
    wrapper.chmod(0o755)
    sources = destination / "sources"
    sources.mkdir()
    shutil.copy2(args.llama_runtime.resolve().parent / "provenance.json", sources / "llama-provenance.json")
    gtk_build = args.gtk_runtime.resolve().parent
    for source_name in ["gtk-4.24.1.tar.xz", "21d3eb76b68d6758122eddaf00b5823f9b2d7f34.patch", "runtime-receipt.json"]:
        shutil.copy2(gtk_build / source_name, sources / source_name)
    shutil.copy2(root / "scripts/build_linux_gtk_runtime.py", sources / "build_linux_gtk_runtime.py")
    shutil.copy2(args.asr_runtime.resolve() / "build-receipt.json", sources / "asr-build-receipt.json")
    for filename in ["install.sh", "CURRENT-PC.md", "com.olledyberg.Koett.desktop", "com.olledyberg.Koett.svg"]:
        shutil.copy2(root / "packaging/linux" / filename, destination / filename)
    for filename in ["README.md", "THIRD_PARTY_NOTICES.md"]:
        shutil.copy2(root / filename, destination / filename)
    shutil.copy2(root.parent / "LICENSE", destination / "LICENSE")
    receipt = {"kind": "local-candidate-not-public-release", "source_revision": revision,
               "source_dirty": dirty, "binary_sha256": digest(binary),
               "binary_bytes": binary.stat().st_size}
    native = root / "native-build/build-receipt.json"
    if native.exists():
        shutil.copy2(native, destination / "native-build-receipt.json")
    (destination / "candidate.json").write_text(json.dumps(receipt, indent=2) + "\n")
    files = {str(path.relative_to(destination)): digest(path)
             for path in sorted(destination.rglob("*")) if path.is_file()}
    (destination / "files.json").write_text(json.dumps(files, indent=2) + "\n")
    archive = output / f"{name}.tar.gz"
    with tarfile.open(archive, "w:gz") as package:
        package.add(destination, arcname=name)
    (output / f"{name}.tar.gz.sha256").write_text(f"{digest(archive)}  {archive.name}\n")
    print(json.dumps({"archive": str(archive), **receipt}, indent=2))


if __name__ == "__main__":
    main()
