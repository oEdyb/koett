#!/usr/bin/env python3
"""Build pinned sherpa-onnx without TTS, then stage only its ASR link inputs."""

import argparse
import hashlib
import json
import os
import re
import time
import urllib.error
import shutil
import subprocess
import tarfile
import urllib.request
from pathlib import Path

from shared_context_patch import PATCHED_SHA256, apply_shared_context

SOURCE_COMMIT = "3dc7c569f31ca2cd4a20ed6f7db780327e6714c5"
SOURCE_SHA256 = "33723a7195bc3d3ecf127dac2675293e578a12fdb1d3de891ff17628c118b66c"
SOURCE_URL = f"https://codeload.github.com/k2-fsa/sherpa-onnx/tar.gz/{SOURCE_COMMIT}"
PREVIOUS_CONTEXT_SHA256 = '673caa2c3c3053158cc5c2fec576b17607ea0a053d648f34422591e0c689d920'
LIBRARIES = (
    "sherpa-onnx-c-api", "sherpa-onnx-core", "kaldi-decoder-core",
    "sherpa-onnx-kaldifst-core", "sherpa-onnx-fstfar", "sherpa-onnx-fst",
    "kaldi-native-fbank-core", "kissfft-float", "onnxruntime", "ssentencepiece_core",
)
# The pinned ONNX Runtime 1.27.1 static archives use different libstdc++ ABIs.
LINUX_CPP_ABIS = {'x86_64-unknown-linux-gnu': '0', 'aarch64-unknown-linux-gnu': '1'}
ONNX_CPP_ABI_SYMBOLS = {
    '0': '_ZN11onnxruntime6common6Status11EmptyStringEv',
    '1': '_ZN11onnxruntime6common6Status11EmptyStringB5cxx11Ev',
}
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
        if target not in LINUX_CPP_ABIS:
            raise ValueError(f"unreviewed Linux native C++ ABI: {target}")
        options["CMAKE_CXX_FLAGS"] = f"-D_GLIBCXX_USE_CXX11_ABI={LINUX_CPP_ABIS[target]}"
    return options


def validate_onnx_cpp_abi(target, symbols):
    expected = LINUX_CPP_ABIS[target]
    defined = {line.split()[-1] for line in symbols.splitlines() if line.split()}
    actual = {abi for abi, symbol in ONNX_CPP_ABI_SYMBOLS.items() if symbol in defined}
    if actual != {expected}:
        raise ValueError(f"ONNX Runtime C++ ABI mismatch for {target}: expected {expected}, found {sorted(actual)}")


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


def download_archive(url, path):
    for attempt in range(3):
        try:
            with urllib.request.urlopen(url, timeout=60) as response, path.open("wb") as output:
                shutil.copyfileobj(response, output)
            return
        except urllib.error.HTTPError as error:
            if error.code not in (429, 502, 503, 504) or attempt == 2:
                raise
            print(f"Temporary source download HTTP {error.code}; retry {attempt + 1} of 2", flush=True)
            time.sleep(2)


def configure_with_download_retries(command):
    # FetchContent downloads ONNX Runtime during configuration. Reconfigure in
    # the same directory so successful verified dependencies remain cached.
    for attempt in range(3):
        result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        print(result.stdout, end="", flush=True)
        if result.returncode == 0:
            return
        temporary_http = re.search(
            r"(?:HTTP/[0-9.]+\s+|requested URL returned error:\s*)(429|502|503|504)\b",
            result.stdout, re.IGNORECASE,
        )
        integrity_failure = re.search(r"hash mismatch|does not match expected|checksum mismatch", result.stdout, re.IGNORECASE)
        if attempt == 2 or not temporary_http or integrity_failure or "download" not in result.stdout.lower():
            raise subprocess.CalledProcessError(result.returncode, command, output=result.stdout)
        print(f"Temporary native dependency download failure; retry {attempt + 1} of 2", flush=True)
        time.sleep(2)

def prepare_context_header(archive, context):
    contents = context.read_text()
    if hashlib.sha256(contents.encode()).hexdigest() == PREVIOUS_CONTEXT_SHA256:
        # The archive has already passed SOURCE_SHA256 validation in prepare_source.
        # Upgrade only this known prior patch; unknown local edits still fail closed.
        member = f'sherpa-onnx-{SOURCE_COMMIT}/sherpa-onnx/csrc/{context.name}'
        with tarfile.open(archive) as package:
            original = package.extractfile(member)
            if original is None:
                raise ValueError('pinned archive has no original NeMo header')
            contents = original.read().decode()
    patched = apply_shared_context(contents).encode()
    if context.read_bytes() != patched:
        context.write_bytes(patched)


def prepare_source(root):
    archive = root / "source.tar.gz"
    if not archive.exists():
        temporary = archive.with_suffix(".download")
        download_archive(SOURCE_URL, temporary)
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
    context = source / 'sherpa-onnx/csrc/offline-recognizer-transducer-nemo-impl.h'
    prepare_context_header(archive, context)
    return source


def stage_libraries(build, root, target):
    destination = root / "lib"
    destination.mkdir(exist_ok=True)
    marker = destination / "koett-asr-only.txt"
    marker.unlink(missing_ok=True)
    capability = destination / 'koett-context.txt'
    capability.unlink(missing_ok=True)
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
    if "linux" in target:
        symbols = subprocess.check_output(
            ["nm", "-g", "--defined-only", str(destination / "libonnxruntime.a")], text=True)
        validate_onnx_cpp_abi(target, symbols)
    receipt = {"schema_version": 1, "source_commit": SOURCE_COMMIT,
               "source_sha256": SOURCE_SHA256, "target": target,
               "cmake_options": native_options(target), "libraries": files}
    context = root / f'sherpa-onnx-{SOURCE_COMMIT}/sherpa-onnx/csrc/offline-recognizer-transducer-nemo-impl.h'
    if context.exists() and sha256(context) == PATCHED_SHA256:
        core = 'sherpa-onnx-core.lib' if 'windows' in target else 'libsherpa-onnx-core.a'
        api = 'sherpa-onnx-c-api.lib' if 'windows' in target else 'libsherpa-onnx-c-api.a'
        if b'koett_context' not in (destination / core).read_bytes():
            raise ValueError('shared-context source was not compiled into the native core archive')
        receipt['shared_context_sha256'] = PATCHED_SHA256
        capability.write_bytes(f'koett-context-v1\npatch={PATCHED_SHA256}\nsource={SOURCE_COMMIT}\ntarget={target}\ncore={files[core]}\napi={files[api]}\n'.encode())
    (root / "build-receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    marker.write_bytes(
        f"koett-asr-only-v1\nsource={SOURCE_COMMIT}\ntarget={target}\n".encode())
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
    configure_with_download_retries(configure_command(source, build, target, args.generator))
    subprocess.run(["cmake", "--build", str(build), "--target", "sherpa-onnx-c-api",
                    "--config", "Release", "--parallel", str(args.jobs)], check=True)
    stage_libraries(build, root, target)


if __name__ == "__main__":
    main()
