#!/usr/bin/env python3
"""Build pinned sherpa-onnx without TTS, then stage only its ASR link inputs."""

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import tarfile
import urllib.request
from pathlib import Path

SOURCE_COMMIT = "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5"
SOURCE_SHA256 = "33723a7195bc3d3ecf127dac2675293e578a12fdb1d3de891ff17628c118b66c"
SOURCE_URL = f"https://codeload.github.com/k2-fsa/sherpa-onnx/tar.gz/{SOURCE_COMMIT}"
LIBRARIES = (
    "sherpa-onnx-c-api", "sherpa-onnx-core", "kaldi-decoder-core",
    "sherpa-onnx-kaldifst-core", "sherpa-onnx-fstfar", "sherpa-onnx-fst",
    "kaldi-native-fbank-core", "kissfft-float", "onnxruntime", "ssentencepiece_core",
)
OPTIONS = {
    "CMAKE_BUILD_TYPE": "Release", "BUILD_SHARED_LIBS": "OFF",
    "SHERPA_ONNX_ENABLE_TTS": "OFF",
    "SHERPA_ONNX_ENABLE_SPEAKER_DIARIZATION": "OFF",
    "SHERPA_ONNX_ENABLE_PORTAUDIO": "OFF",
    "SHERPA_ONNX_ENABLE_WEBSOCKET": "OFF",
    "SHERPA_ONNX_ENABLE_BINARY": "OFF",
    "SHERPA_ONNX_BUILD_C_API_EXAMPLES": "OFF",
    "SHERPA_ONNX_USE_PRE_INSTALLED_ONNXRUNTIME_IF_AVAILABLE": "OFF",
    "SHERPA_ONNX_LINK_LIBSTDCPP_STATICALLY": "OFF",
}


def sha256(path):
    with path.open("rb") as stream:
        digest = hashlib.sha256()
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
        return digest.hexdigest()


def native_options(target):
    options = dict(OPTIONS)
    if "linux" in target:
        # Upstream's manylinux ONNX archive uses the original libstdc++ ABI.
        # Mixing both regex ABIs can corrupt memory (GCC PR libstdc++/118408).
        options["CMAKE_CXX_FLAGS"] = "-D_GLIBCXX_USE_CXX11_ABI=0"
    return options


def configure_command(source, build, target, generator=None):
    command = ["cmake", "-S", str(source), "-B", str(build)]
    if "windows-msvc" in target:
        if not target.startswith("x86_64-"):
            raise ValueError(f"unsupported Windows native target: {target}")
        if generator:
            command.extend(["-G", generator])
        command.extend(["-A", "x64"])
    else:
        command.extend(["-G", generator or "Ninja"])
    return command + [f"-D{key}={value}" for key, value in native_options(target).items()]


def remove_disabled_tts_stubs(contents):
    """Keep the enabled implementation guarded; remove only the disabled else."""
    start = contents.index("#if SHERPA_ONNX_ENABLE_TTS == 1\nstruct SherpaOnnxOfflineTts")
    lines = contents[start:].splitlines(keepends=True)
    depth = 0
    else_offset = None
    position = start
    for line in lines:
        directive = line.strip()
        if directive.startswith(("#if ", "#ifdef ", "#ifndef ")):
            depth += 1
        elif directive.startswith("#else") and depth == 1:
            else_offset = position
        elif directive.startswith("#endif"):
            depth -= 1
            if depth == 0:
                if else_offset is None:
                    raise ValueError("pinned TTS disabled branch is missing")
                return contents[:else_offset] + contents[position:]
        position += len(line)
    raise ValueError("pinned TTS guard is not balanced")


def prepare_source(root):
    archive = root / "source.tar.gz"
    if not archive.exists():
        temporary = archive.with_suffix(".download")
        with urllib.request.urlopen(SOURCE_URL, timeout=60) as response, temporary.open("wb") as output:
            shutil.copyfileobj(response, output)
        if sha256(temporary) != SOURCE_SHA256:
            raise ValueError("sherpa source checksum mismatch")
        temporary.replace(archive)
    if sha256(archive) != SOURCE_SHA256:
        raise ValueError("cached sherpa source checksum mismatch")
    source = root / f"sherpa-onnx-{SOURCE_COMMIT}"
    extracted = source / ".koett-source-sha256"
    if not extracted.exists() or extracted.read_text().strip() != SOURCE_SHA256:
        with tarfile.open(archive) as package:
            # Upstream's example links are unnecessary for the native C API.
            # Excluding them also works with older Python data-filter versions.
            package.extractall(root, members=(member for member in package
                                             if member.isfile() or member.isdir()),
                               filter="data")
        extracted.write_text(SOURCE_SHA256 + "\n")
    api = source / "sherpa-onnx/c-api/c-api.cc"
    original = api.with_suffix(".cc.original")
    if not original.exists():
        shutil.copyfile(api, original)
    api.write_text(remove_disabled_tts_stubs(original.read_text()))
    return source


def stage_libraries(build, root, target):
    destination = root / "lib"
    destination.mkdir(exist_ok=True)
    for path in destination.iterdir():
        if any(term in path.name.lower() for term in ("piper", "espeak", "ucd")):
            raise ValueError(f"forbidden native component: {path.name}")
    files = {}
    for name in LIBRARIES:
        filename = f"{name}.lib" if "windows" in target else f"lib{name}.a"
        candidates = list(build.rglob(filename))
        if len(candidates) != 1:
            raise ValueError(f"expected exactly one {filename}, found {len(candidates)}")
        shutil.copy2(candidates[0], destination / filename)
        files[filename] = sha256(destination / filename)
    receipt = {"schema_version": 1, "source_commit": SOURCE_COMMIT,
               "source_sha256": SOURCE_SHA256, "target": target,
               "cmake_options": native_options(target), "libraries": files}
    (root / "build-receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    (destination / "koett-asr-only.txt").write_text(
        f"koett-asr-only-v1\nsource={SOURCE_COMMIT}\ntarget={target}\n")
    print(json.dumps(receipt, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path,
                        default=Path(__file__).resolve().parents[1] / "native-build")
    parser.add_argument("--jobs", type=int, default=min(os.cpu_count() or 2, 8))
    parser.add_argument("--generator", help="CMake generator; Windows defaults to installed Visual Studio")
    args = parser.parse_args()
    host = subprocess.check_output(["rustc", "-vV"], text=True)
    target = next(line.removeprefix("host: ") for line in host.splitlines()
                  if line.startswith("host: "))
    root = args.root.resolve()
    root.mkdir(parents=True, exist_ok=True)
    source = prepare_source(root)
    build = root / "build"
    subprocess.run(configure_command(source, build, target, args.generator), check=True)
    subprocess.run(["cmake", "--build", str(build), "--target", "sherpa-onnx-c-api",
                    "--config", "Release", "--parallel", str(args.jobs)], check=True)
    stage_libraries(build, root, target)


if __name__ == "__main__":
    main()
