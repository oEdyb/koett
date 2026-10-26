#!/usr/bin/env python3
"""Package a local Linux candidate without installing it or publishing a release."""

import argparse
import hashlib
import json
from pathlib import Path
import platform
import shutil
import subprocess
import tarfile


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
    shutil.copy2(binary, destination / "koett")
    for filename in ["install.sh", "com.olledyberg.Koett.desktop", "com.olledyberg.Koett.svg"]:
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
    archive = output / f"{name}.tar.gz"
    with tarfile.open(archive, "w:gz") as package:
        package.add(destination, arcname=name)
    (output / f"{name}.tar.gz.sha256").write_text(f"{digest(archive)}  {archive.name}\n")
    print(json.dumps({"archive": str(archive), **receipt}, indent=2))


if __name__ == "__main__":
    main()
