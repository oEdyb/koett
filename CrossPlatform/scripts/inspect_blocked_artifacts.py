#!/usr/bin/env python3
"""Download and inspect Koett's blocked Windows and Linux control artifacts."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import struct
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
import zipfile
from pathlib import Path
from typing import Iterable


SIGNALS = {
    "espeak": re.compile(
        r"(?:espeak(?:-ng|_ng)?(?:[_@-]|\b)|ESPEAK_DATA_PATH)", re.IGNORECASE
    ),
    "piper": re.compile(r"(?:piper[_ -]?phonem|@piper@@|\bpiper\b)", re.IGNORECASE),
    "tts": re.compile(
        r"(?:SherpaOnnx\w*Tts|OfflineTts|sherpa[-_ ]onnx\w*tts)", re.IGNORECASE
    ),
}
MAX_SAMPLES_PER_SIGNAL = 40


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def printable_strings(data: bytes, minimum: int = 4) -> list[str]:
    ascii_pattern = re.compile(rb"[\x20-\x7e]{%d,}" % minimum)
    utf16_pattern = re.compile(
        rb"(?<![\x20-\x7e])(?:[\x20-\x7e]\x00){%d,}" % minimum
    )
    values = [match.decode("ascii") for match in ascii_pattern.findall(data)]
    values.extend(match.decode("utf-16le") for match in utf16_pattern.findall(data))
    return values


def signal_evidence(lines: Iterable[str]) -> dict[str, dict[str, object]]:
    matches: dict[str, dict[str, object]] = {}
    unique_lines = sorted(set(lines))
    for name, pattern in SIGNALS.items():
        found = [line for line in unique_lines if pattern.search(line)]
        matches[name] = {
            "match_count": len(found),
            "samples": found[:MAX_SAMPLES_PER_SIGNAL],
            "samples_truncated": len(found) > MAX_SAMPLES_PER_SIGNAL,
        }
    return matches


def parse_objdump(output: str) -> tuple[list[str], dict[str, dict[str, object]]]:
    imports = []
    for line in output.splitlines():
        stripped = line.strip()
        if stripped.startswith("DLL Name:"):
            imports.append(stripped.split(":", 1)[1].strip())
        elif stripped.startswith("NEEDED"):
            imports.append(stripped.split(maxsplit=1)[1])
    return sorted(set(imports)), signal_evidence(output.splitlines())


def binary_format(path: Path) -> str:
    with path.open("rb") as source:
        header = source.read(64)
        if header.startswith(b"MZ") and len(header) == 64:
            pe_offset = struct.unpack_from("<I", header, 0x3C)[0]
            source.seek(pe_offset)
            pe_header = source.read(26)
            if pe_header[:4] != b"PE\0\0" or len(pe_header) != 26:
                return "unknown"
            machine = struct.unpack_from("<H", pe_header, 4)[0]
            optional_magic = struct.unpack_from("<H", pe_header, 24)[0]
            if machine == 0x8664 and optional_magic == 0x20B:
                return "PE32+-x86_64"
            return f"PE-machine-{machine}-optional-{optional_magic}"
    if header.startswith(b"\x7fELF") and len(header) >= 20 and header[4] == 2:
        byte_order = "<" if header[5] == 1 else ">"
        machine = struct.unpack(f"{byte_order}H", header[18:20])[0]
        names = {0x3E: "ELF64-x86_64", 0xB7: "ELF64-aarch64"}
        return names.get(machine, f"ELF-machine-{machine}")
    return "unknown"


def command_version(command: str) -> str:
    result = subprocess.run(
        [command, "--version"],
        check=True,
        capture_output=True,
        text=True,
        env={**os.environ, "LC_ALL": "C"},
    )
    return result.stdout.splitlines()[0].strip()


def executable_info(path: Path, version: str) -> dict[str, object]:
    resolved = path.resolve()
    return {
        "path": str(resolved),
        "version": version,
        "bytes": resolved.stat().st_size,
        "sha256": sha256_file(resolved),
    }


def verify_inspection_toolchain(control: dict[str, object]) -> dict[str, object]:
    objdump_path = Path(shutil.which("objdump") or "")
    actual = {
        "python": executable_info(
            Path(sys.executable), ".".join(str(part) for part in sys.version_info[:3])
        ),
        "objdump": executable_info(objdump_path, command_version("objdump")),
    }
    if actual != control["inspection_toolchain"]:
        raise ValueError(
            "inspection toolchain changed; create and review a new control instead of overwriting this one"
        )
    return actual


def verify_mac_model_manifest(
    control: dict[str, object], control_path: Path
) -> dict[str, object]:
    expected = control["mac_model_cache_manifest"]
    manifest_path = control_path.parent / str(expected["file"])
    verify(
        manifest_path,
        int(expected["bytes"]),
        str(expected["sha256"]),
        "macOS model cache manifest",
    )
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if manifest.get("schema_version") != 1 or not manifest.get("models"):
        raise ValueError("macOS model cache manifest has an unsupported schema")
    model_ids: set[str] = set()
    for model in manifest["models"]:
        model_id = str(model["id"])
        if model_id in model_ids:
            raise ValueError(f"duplicate macOS model in manifest: {model_id}")
        model_ids.add(model_id)
        file_paths = [str(item["path"]) for item in model["files"]]
        if file_paths != sorted(file_paths) or len(file_paths) != len(set(file_paths)):
            raise ValueError(f"model files are not unique and sorted: {model_id}")
        for item in model["files"]:
            file_path = Path(str(item["path"]))
            if file_path.is_absolute() or ".." in file_path.parts:
                raise ValueError(f"unsafe model manifest path: {item['path']}")
            if int(item["bytes"]) < 0 or not re.fullmatch(
                r"[0-9a-f]{64}", str(item["sha256"])
            ):
                raise ValueError(f"invalid model file identity: {item['path']}")
    return {
        "file": expected["file"],
        "bytes": manifest_path.stat().st_size,
        "sha256": sha256_file(manifest_path),
        "model_ids": sorted(model_ids),
        "verified": True,
    }


def run_objdump(binary: Path) -> str:
    result = subprocess.run(
        ["objdump", "-p", str(binary)],
        check=True,
        capture_output=True,
        text=True,
        env={**os.environ, "LC_ALL": "C"},
    )
    return result.stdout


def safe_destination(root: Path, member_name: str) -> Path:
    destination = (root / member_name).resolve()
    if destination != root.resolve() and root.resolve() not in destination.parents:
        raise ValueError(f"archive entry escapes destination: {member_name}")
    return destination


def extract_zip(archive: Path, destination: Path) -> None:
    with zipfile.ZipFile(archive) as package:
        for member in package.infolist():
            safe_destination(destination, member.filename)
        package.extractall(destination)


def extract_tar(archive: Path, destination: Path) -> None:
    with tarfile.open(archive, "r:gz") as package:
        for member in package.getmembers():
            safe_destination(destination, member.name)
            if member.issym() or member.islnk():
                raise ValueError(f"archive contains a link: {member.name}")
            if not member.isfile() and not member.isdir():
                raise ValueError(f"archive contains an unsupported entry: {member.name}")
        package.extractall(destination)


def verify(path: Path, expected_bytes: int, expected_sha256: str, label: str) -> None:
    actual_bytes = path.stat().st_size
    actual_sha256 = sha256_file(path)
    if actual_bytes != expected_bytes:
        raise ValueError(f"{label} byte count is {actual_bytes}, expected {expected_bytes}")
    if actual_sha256 != expected_sha256:
        raise ValueError(f"{label} SHA-256 is {actual_sha256}, expected {expected_sha256}")


def download_artifact(repository: str, artifact_id: int, destination: Path) -> None:
    with destination.open("wb") as output:
        subprocess.run(
            ["gh", "api", f"repos/{repository}/actions/artifacts/{artifact_id}/zip"],
            check=True,
            stdout=output,
        )


def download_release_asset(repository: str, asset_id: int, destination: Path) -> None:
    with destination.open("wb") as output:
        subprocess.run(
            [
                "gh",
                "api",
                "-H",
                "Accept: application/octet-stream",
                f"repos/{repository}/releases/assets/{asset_id}",
            ],
            check=True,
            stdout=output,
        )


def github_json(endpoint: str) -> object:
    result = subprocess.run(
        ["gh", "api", endpoint],
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(result.stdout)


def download_url(url: str, destination: Path) -> None:
    request = urllib.request.Request(url, headers={"User-Agent": "Koett artifact audit"})
    with urllib.request.urlopen(request, timeout=60) as response, destination.open("wb") as output:
        shutil.copyfileobj(response, output, length=1024 * 1024)


def verify_sherpa_inputs(control: dict[str, object], work: Path) -> dict[str, object]:
    sherpa = control["sherpa_onnx_1_13_5"]
    tag = github_json(
        f"repos/{sherpa['repository']}/git/ref/tags/{sherpa['source_tag']}"
    )
    tag_object = tag["object"]
    if tag_object["type"] != "commit" or tag_object["sha"] != sherpa["source_commit"]:
        raise ValueError("sherpa source tag does not resolve to the pinned commit")
    release = github_json(
        f"repos/{sherpa['repository']}/releases/tags/{sherpa['source_tag']}"
    )
    release_assets = {int(asset["id"]): asset for asset in release["assets"]}

    crate = work / "sherpa-onnx-sys-1.13.5.crate"
    download_url(str(sherpa["crate_url"]), crate)
    verify(
        crate,
        int(sherpa["crate_bytes"]),
        str(sherpa["crate_checksum"]),
        "sherpa-onnx-sys crate",
    )
    with tarfile.open(crate, "r:gz") as package:
        build_member = package.getmember(str(sherpa["build_rs_path"]))
        extracted = package.extractfile(build_member)
        if extracted is None:
            raise ValueError("sherpa-onnx-sys crate has no build.rs data")
        build_data = extracted.read()
    build_sha256 = hashlib.sha256(build_data).hexdigest()
    if build_sha256 != sherpa["build_rs_sha256"]:
        raise ValueError("sherpa-onnx-sys build.rs SHA-256 changed")
    build_text = build_data.decode("utf-8")
    requested_libraries = sherpa["static_libraries_requested_by_build_script"]
    for library in requested_libraries:
        if f'"{library}"' not in build_text:
            raise ValueError(f"sherpa build.rs no longer requests {library}")

    archive_reports = []
    for archive in sherpa["native_archives_used_by_build_script"]:
        release_asset = release_assets.get(int(archive["asset_id"]))
        expected_digest = f"sha256:{archive['sha256']}"
        if (
            release_asset is None
            or release_asset["name"] != archive["name"]
            or int(release_asset["size"]) != int(archive["bytes"])
            or release_asset["digest"] != expected_digest
        ):
            raise ValueError(f"GitHub release metadata changed for {archive['name']}")
        archive_path = work / str(archive["name"])
        download_release_asset(
            str(sherpa["repository"]), int(archive["asset_id"]), archive_path
        )
        verify(
            archive_path,
            int(archive["bytes"]),
            str(archive["sha256"]),
            f"native archive {archive['name']}",
        )
        with tarfile.open(archive_path, "r:bz2") as package:
            members = sorted(member.name for member in package.getmembers())
        component_evidence = signal_evidence(members)
        if not component_evidence["espeak"]["match_count"]:
            raise ValueError(f"{archive['name']} does not contain the expected eSpeak archive")
        if not component_evidence["piper"]["match_count"]:
            raise ValueError(f"{archive['name']} does not contain the expected Piper archive")
        archive_reports.append(
            {
                "name": archive["name"],
                "asset_id": archive["asset_id"],
                "bytes": archive_path.stat().st_size,
                "sha256": sha256_file(archive_path),
                "component_evidence": component_evidence,
            }
        )
    return {
        "schema_version": 1,
        "crate": {
            "bytes": crate.stat().st_size,
            "sha256": sha256_file(crate),
            "build_rs_sha256": build_sha256,
        },
        "source_commit": sherpa["source_commit"],
        "source_tag": sherpa["source_tag"],
        "release_id": release["id"],
        "static_libraries_requested_by_build_script": requested_libraries,
        "native_archives": archive_reports,
        "verified": True,
    }


def verify_ci_output(control: dict[str, object], work: Path) -> dict[str, object]:
    ci = control["ci_logs"]
    archive = work / "ci-run-logs.zip"
    with archive.open("wb") as output:
        subprocess.run(
            [
                "gh",
                "api",
                f"repos/{control['repository']}/actions/runs/{control['cross_platform']['ci_run_id']}/logs",
            ],
            check=True,
            stdout=output,
        )
    verify(archive, int(ci["bytes"]), str(ci["sha256"]), "CI log archive")
    log_directory = work / "ci-logs"
    log_directory.mkdir()
    extract_zip(archive, log_directory)
    jobs = []
    for job in ci["jobs"]:
        log = log_directory / str(job["log_file"])
        verify(log, int(job["log_bytes"]), str(job["log_sha256"]), "CI job log")
        text = log.read_text(encoding="utf-8")
        for expected in job["required_outputs"]:
            if expected not in text:
                raise ValueError(f"CI log {job['log_file']} is missing: {expected}")
        jobs.append(
            {
                "job_id": job["job_id"],
                "name": job["name"],
                "log_bytes": log.stat().st_size,
                "log_sha256": sha256_file(log),
                "required_outputs": job["required_outputs"],
            }
        )
    return {
        "schema_version": 1,
        "run_id": control["cross_platform"]["ci_run_id"],
        "run_attempt": control["cross_platform"]["ci_run_attempt"],
        "head_commit": control["cross_platform"]["commit"],
        "log_archive_bytes": archive.stat().st_size,
        "log_archive_sha256": sha256_file(archive),
        "jobs": jobs,
        "verified": True,
    }


def artifact_verdict(matched: list[str], source_verified: bool) -> str:
    return "blocked" if matched and source_verified else "needs-review"


def inspect_artifact(
    control: dict[str, object],
    artifact: dict[str, object],
    work: Path,
    sherpa_inputs: dict[str, object],
    mac_model_manifest: dict[str, object],
    tools: dict[str, object],
) -> dict[str, object]:
    outer_archive = work / f"{artifact['name']}.github.zip"
    download_artifact(str(control["repository"]), int(artifact["artifact_id"]), outer_archive)
    verify(
        outer_archive,
        int(artifact["github_archive_bytes"]),
        str(artifact["github_archive_sha256"]),
        "GitHub artifact archive",
    )

    outer_directory = work / f"{artifact['name']}-outer"
    outer_directory.mkdir()
    extract_zip(outer_archive, outer_directory)
    package = outer_directory / str(artifact["package_file"])
    verify(
        package,
        int(artifact["package_bytes"]),
        str(artifact["package_sha256"]),
        "packaged app archive",
    )

    package_directory = work / f"{artifact['name']}-package"
    package_directory.mkdir()
    if package.suffix == ".zip":
        extract_zip(package, package_directory)
    elif package.name.endswith(".tar.gz"):
        extract_tar(package, package_directory)
    else:
        raise ValueError(f"unsupported package type: {package.name}")

    binary = package_directory / str(artifact["binary_path"])
    verify(
        binary,
        int(artifact["binary_bytes"]),
        str(artifact["binary_sha256"]),
        "Koett binary",
    )
    objdump_output = run_objdump(binary)
    imports, symbol_matches = parse_objdump(objdump_output)
    string_matches = signal_evidence(printable_strings(binary.read_bytes()))
    matched = sorted(
        name
        for name in SIGNALS
        if int(symbol_matches[name]["match_count"]) > 0
        or int(string_matches[name]["match_count"]) > 0
    )

    return {
        "schema_version": 1,
        "control_commit": control["cross_platform"]["commit"],
        "workflow_run": control["cross_platform"]["ci_run"],
        "artifact": {
            "id": artifact["artifact_id"],
            "name": artifact["name"],
            "github_archive_bytes": outer_archive.stat().st_size,
            "github_archive_sha256": sha256_file(outer_archive),
            "package_bytes": package.stat().st_size,
            "package_sha256": sha256_file(package),
            "binary_path": artifact["binary_path"],
            "binary_bytes": binary.stat().st_size,
            "binary_sha256": sha256_file(binary),
            "binary_format": binary_format(binary),
        },
        "inspection": {
            "tools": tools,
            "commands": [
                ["objdump", "-p", str(artifact["binary_path"])],
                ["python-standard-library", "ASCII-and-UTF-16LE-string-scan"],
            ],
            "imported_libraries": imports,
            "symbol_or_header_evidence": symbol_matches,
            "string_evidence": string_matches,
            "matched_indicators": matched,
        },
        "source_linkage_evidence": sherpa_inputs,
        "mac_model_cache_manifest": mac_model_manifest,
        "verdict": artifact_verdict(matched, bool(sherpa_inputs["verified"])),
        "limitations": [
            "Imported-library inspection does not identify code linked from static archives.",
            "String matches are supporting evidence and are not sufficient alone.",
            "The verified native inputs and callable/exported symbol evidence provide the stronger provenance evidence.",
        ],
    }


def write_json(path: Path, value: object) -> None:
    serialized = json.dumps(value, indent=2, sort_keys=True) + "\n"
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(serialized, encoding="utf-8")
    temporary.replace(path)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--control", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    arguments = parser.parse_args()

    if shutil.which("gh") is None or shutil.which("objdump") is None:
        raise SystemExit("This inspection needs gh and objdump on PATH.")

    control_path = arguments.control.resolve()
    control = json.loads(control_path.read_text(encoding="utf-8"))
    arguments.output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="koett-artifacts-") as temporary:
        work = Path(temporary)
        tools = verify_inspection_toolchain(control)
        mac_model_manifest = verify_mac_model_manifest(control, control_path)
        sherpa_inputs = verify_sherpa_inputs(control, work)
        ci_output = verify_ci_output(control, work)
        write_json(arguments.output / "sherpa-inputs.json", sherpa_inputs)
        write_json(arguments.output / "ci-output.json", ci_output)
        for artifact in control["artifacts"]:
            report = inspect_artifact(
                control, artifact, work, sherpa_inputs, mac_model_manifest, tools
            )
            write_json(arguments.output / str(artifact["report_file"]), report)


if __name__ == "__main__":
    main()
