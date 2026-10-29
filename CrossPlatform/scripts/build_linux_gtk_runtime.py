#!/usr/bin/env python3
"""Build GTK 4.24.1 with its upstream root accessibility fix into a private prefix."""

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import tarfile
import tempfile
from pathlib import Path
from urllib.request import urlopen

VERSION = "4.24.1"
ARCHIVE_SHA = "e98abe720e16129c8c0f50761dee0a2e9ae2478055e31018f6cf977fcf6513e9"
FIX = "21d3eb76b68d6758122eddaf00b5823f9b2d7f34"
PATCH_SHA = "f2d4b12c7dcd5bd98cf20df429fac67ba764fb0dac42271f4d58eafb06840535"
PROTOCOLS = "02e63e74a807afed95bc25a386173110afef24e3"


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def download(url, path, expected):
    if not path.exists():
        partial = path.with_suffix(path.suffix + ".part")
        with urlopen(url, timeout=60) as response, partial.open("wb") as target:
            shutil.copyfileobj(response, target)
        if digest(partial) != expected:
            partial.unlink()
            raise ValueError(f"Checksum mismatch: {url}")
        partial.rename(path)
    if digest(path) != expected:
        raise ValueError(f"Cached input checksum mismatch: {path}")


def run(arguments, **kwargs):
    subprocess.run([str(value) for value in arguments], check=True, **kwargs)


def output(arguments, **kwargs):
    return subprocess.check_output([str(value) for value in arguments], text=True, **kwargs).strip()


def source_fingerprint(source):
    entries = {}
    for path in source.rglob("*"):
        name = str(path.relative_to(source))
        if path.is_symlink():
            entries[name] = ["symlink", os.readlink(path)]
        elif path.is_file():
            entries[name] = ["file", digest(path), path.stat().st_mode & 0o111]
        elif path.is_dir():
            entries[name] = ["directory"]
        else:
            raise ValueError(f"Unexpected source entry: {path}")
    return entries


def verify_source(root, archive, patch):
    source = root / f"gtk-{VERSION}"
    with tempfile.TemporaryDirectory(prefix=".gtk-source-check-", dir=root) as temporary:
        with tarfile.open(archive) as bundle:
            bundle.extractall(temporary, filter="data")
        pristine = Path(temporary) / source.name
        run(["git", "apply", "--check", patch], cwd=pristine)
        run(["git", "apply", patch], cwd=pristine)
        expected = source_fingerprint(pristine)
        if source.exists():
            actual = source_fingerprint(source)
            if actual != expected:
                changed = sorted(name for name in actual.keys() | expected.keys()
                                 if actual.get(name) != expected.get(name))
                raise ValueError(f"Cached GTK source differs from verified archive and patch: {changed[:5]}")
        else:
            shutil.copytree(pristine, source, symlinks=True)
    return hashlib.sha256(json.dumps(expected, sort_keys=True).encode()).hexdigest()


def verify_build(build, source, prefix, expected=None):
    settings = json.loads((build / "meson-info/intro-buildoptions.json").read_text())
    actual = {item["name"]: item["value"] for item in settings}
    for name, value in {"prefix": str(prefix), **(expected or {})}.items():
        if actual.get(name) != value:
            raise ValueError(f"Existing build option differs: {name}")
    files = json.loads((build / "meson-info/intro-buildsystem_files.json").read_text())
    if (source / "meson.build").resolve() not in {Path(path).resolve() for path in files}:
        raise ValueError("Existing build uses a different source directory")
    return actual


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work-directory", type=Path, required=True)
    args = parser.parse_args()
    root = args.work_directory.absolute()
    root.mkdir(mode=0o700, parents=True, exist_ok=True)
    for name in [f"gtk-{VERSION}", "wayland-protocols", "protocols-build", "protocols-runtime", "build", "runtime"]:
        if not (root / name).resolve().is_relative_to(root.resolve()):
            raise ValueError(f"Build path escapes the private work directory: {name}")
    archive = root / f"gtk-{VERSION}.tar.xz"
    archive_url = f"https://download.gnome.org/sources/gtk/4.24/{archive.name}"
    patch = root / f"{FIX}.patch"
    download(archive_url, archive, ARCHIVE_SHA)
    download(f"https://github.com/GNOME/gtk/commit/{FIX}.patch", patch, PATCH_SHA)
    source = root / f"gtk-{VERSION}"
    tree_sha = verify_source(root, archive, patch)
    protocols = root / "wayland-protocols"
    if not protocols.exists():
        run(["git", "clone", "--depth", "1", "--branch", "1.48", "--single-branch",
             "https://gitlab.freedesktop.org/wayland/wayland-protocols.git", protocols])
    if output(["git", "rev-parse", "HEAD"], cwd=protocols) != PROTOCOLS:
        raise ValueError("Unexpected wayland-protocols revision")
    run(["git", "diff", "--quiet", "--no-ext-diff", "HEAD"], cwd=protocols)
    if output(["git", "ls-files", "--others"], cwd=protocols):
        raise ValueError("Unexpected untracked wayland-protocols input")
    protocol_build = root / "protocols-build"
    if not (protocol_build / "build.ninja").exists():
        run(["meson", "setup", protocol_build, protocols,
             f"--prefix={root / 'protocols-runtime'}", "-Dtests=false"])
    verify_build(protocol_build, protocols, root / "protocols-runtime", {"tests": False})
    run(["meson", "install", "-C", protocol_build])
    environment = {**os.environ, "PKG_CONFIG_PATH": str(root / "protocols-runtime/share/pkgconfig"),
                   "PYTHONDONTWRITEBYTECODE": "1"}
    build = root / "build"
    prefix = root / "runtime"
    options = ["--buildtype=release", "--wrap-mode=nofallback", "-Dbuild-demos=false",
               "-Dbuild-testsuite=false", "-Dbuild-examples=false", "-Dbuild-tests=false",
               "-Ddocumentation=false", "-Dman-pages=false", "-Dintrospection=disabled"]
    if not (build / "build.ninja").exists():
        run(["meson", "setup", build, source, f"--prefix={prefix}", *options], env=environment)
    actual_options = verify_build(build, source, prefix, {
        "buildtype": "release", "wrap_mode": "nofallback", "build-demos": False,
        "build-testsuite": False, "build-examples": False, "build-tests": False,
        "documentation": False, "man-pages": False, "introspection": "disabled",
        "x11-backend": True, "wayland-backend": True,
    })
    run(["systemd-run", "--user", "--scope", "--slice=agents-lab.slice",
         f"--unit=koett-gtk-compile-{os.getpid()}", "-p", "CPUQuota=200%", "-p", "MemoryMax=4G",
         "ninja", "-C", build, "-j2"], env=environment)
    run(["meson", "install", "-C", build, "--no-rebuild", "--tags", "runtime"], env=environment)
    library = prefix / "lib/libgtk-4.so.1"
    dependencies = ["glib-2.0", "pango", "cairo", "gdk-pixbuf-2.0", "graphene-gobject-1.0",
                    "wayland-client", "vulkan"]
    compiler = json.loads((build / "meson-info/intro-compilers.json").read_text())["host"]["c"]
    receipt = {"version": VERSION, "source": archive_url, "source_sha256": ARCHIVE_SHA,
               "patch_commit": FIX, "patch_sha256": PATCH_SHA, "protocols_commit": PROTOCOLS,
               "library": str(library.resolve()), "library_sha256": digest(library),
               "prefix": str(prefix), "meson_options": actual_options, "source_tree_sha256": tree_sha,
               "compiler": compiler,
               "dependencies": {name: output(["pkg-config", "--modversion", name]) for name in dependencies},
               "dynamic_libraries": output(["ldd", library]),
               "scope": "Current-host candidate; not a portable Linux distribution runtime"}
    licenses = prefix / "share/licenses/gtk4"
    licenses.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source / "COPYING", licenses / "COPYING")
    shutil.copy2(patch, licenses / patch.name)
    (root / "runtime-receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    (licenses / "provenance.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(f"Verified private GTK runtime: {library}", flush=True)


if __name__ == "__main__":
    main()
