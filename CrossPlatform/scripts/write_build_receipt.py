#!/usr/bin/env python3
"""Write a package checksum and an unsigned, deterministic CI build receipt."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
from pathlib import Path
from typing import Mapping


CI_FIELDS = (
    "GITHUB_EVENT_NAME",
    "GITHUB_JOB",
    "GITHUB_REF",
    "GITHUB_REPOSITORY",
    "GITHUB_RUN_ATTEMPT",
    "GITHUB_RUN_ID",
    "GITHUB_SHA",
    "GITHUB_WORKFLOW",
    "GITHUB_WORKFLOW_SHA",
    "RUNNER_ARCH",
    "RUNNER_ENVIRONMENT",
    "RUNNER_NAME",
    "RUNNER_OS",
)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def command_version(command: str) -> str:
    result = subprocess.run(
        [command, "--version", "--verbose"],
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip()


def build_receipt(
    artifact: Path,
    target: str,
    environment: Mapping[str, str],
    rustc_version: str,
    cargo_version: str,
) -> dict[str, object]:
    if not target:
        raise ValueError("target must not be empty")
    if not artifact.is_file() or artifact.stat().st_size == 0:
        raise ValueError("artifact must be a nonempty file")
    if environment.get("GITHUB_ACTIONS") == "true":
        missing = [name for name in CI_FIELDS if not environment.get(name)]
        if missing:
            raise ValueError(f"missing GitHub Actions fields: {', '.join(missing)}")
    commit = environment.get("GITHUB_SHA", "")
    workflow_commit = environment.get("GITHUB_WORKFLOW_SHA", "")
    if not re.fullmatch(r"[0-9a-fA-F]{40}", commit):
        raise ValueError("source commit must be a full Git SHA")
    if not re.fullmatch(r"[0-9a-fA-F]{40}", workflow_commit):
        raise ValueError("workflow commit must be a full Git SHA")

    return {
        "attestation_status": "unsigned-build-receipt-only",
        "build": {
            "job": environment.get("GITHUB_JOB", "local"),
            "run_attempt": environment.get("GITHUB_RUN_ATTEMPT", "local"),
            "run_id": environment.get("GITHUB_RUN_ID", "local"),
            "runner": {
                "architecture": environment.get("RUNNER_ARCH", "local"),
                "environment": environment.get("RUNNER_ENVIRONMENT", "local"),
                "image_os": environment.get("ImageOS", "unknown"),
                "image_version": environment.get("ImageVersion", "unknown"),
                "name": environment.get("RUNNER_NAME", "local"),
                "operating_system": environment.get("RUNNER_OS", "local"),
            },
            "target": target,
            "tools": {"cargo": cargo_version, "rustc": rustc_version},
        },
        "schema_version": 1,
        "source": {
            "commit": commit.lower(),
            "event": environment.get("GITHUB_EVENT_NAME", "local"),
            "ref": environment.get("GITHUB_REF", "local"),
            "repository": environment.get("GITHUB_REPOSITORY", "local"),
            "workflow": environment.get("GITHUB_WORKFLOW", "local"),
            "workflow_commit": workflow_commit.lower(),
        },
        "subject": {
            "bytes": artifact.stat().st_size,
            "name": artifact.name,
            "sha256": sha256_file(artifact),
        },
    }


def write_text(path: Path, content: str) -> None:
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(content, encoding="utf-8")
    temporary.replace(path)


def write_evidence(
    artifact: Path,
    output_directory: Path,
    target: str,
    environment: Mapping[str, str],
    rustc_version: str,
    cargo_version: str,
) -> tuple[Path, Path]:
    receipt = build_receipt(
        artifact, target, environment, rustc_version, cargo_version
    )
    output_directory.mkdir(parents=True, exist_ok=True)
    checksum_path = output_directory / f"{artifact.name}.sha256"
    receipt_path = output_directory / f"{artifact.name}.build-receipt.json"
    write_text(
        checksum_path,
        f"{receipt['subject']['sha256']}  {artifact.name}\n",
    )
    write_text(receipt_path, json.dumps(receipt, indent=2, sort_keys=True) + "\n")
    return checksum_path, receipt_path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifact", required=True, type=Path)
    parser.add_argument("--target", required=True)
    parser.add_argument("--output-directory", required=True, type=Path)
    arguments = parser.parse_args()
    write_evidence(
        arguments.artifact,
        arguments.output_directory,
        arguments.target,
        os.environ,
        command_version("rustc"),
        command_version("cargo"),
    )


if __name__ == "__main__":
    main()
