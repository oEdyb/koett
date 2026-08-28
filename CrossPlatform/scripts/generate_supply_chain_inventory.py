#!/usr/bin/env python3
"""Generate Koett's deterministic source inventory and draft CycloneDX SBOM."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
from collections import namedtuple
from pathlib import Path
from typing import Any
from urllib.parse import quote

Bundle = namedtuple("Bundle", ["inventory", "sbom", "gaps"])

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
CROSS_PLATFORM = REPOSITORY_ROOT / "CrossPlatform"
POLICY_PATH = CROSS_PLATFORM / "supply-chain" / "inventory-policy.json"
OUTPUT_DIRECTORY = CROSS_PLATFORM / "evidence" / "supply-chain-inventory"
OUTPUTS = {
    "inventory": OUTPUT_DIRECTORY / "inventory.json",
    "sbom": OUTPUT_DIRECTORY / "koett-source.cdx.json",
    "gaps": OUTPUT_DIRECTORY / "notice-gaps.json",
}
HEX_64 = re.compile(r"^[0-9a-f]{64}$")
LOCK_VALUE = re.compile(r'^([a-z_]+) = ("(?:[^"\\]|\\.)*")$')
LICENSE_FILE_PREFIXES = ("copying", "copyright", "license", "notice", "unlicense")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def file_record(path: Path, display_path: str | None = None) -> dict[str, Any]:
    return {
        "path": display_path or path.name,
        "bytes": path.stat().st_size,
        "sha256": sha256_file(path),
    }


def parse_cargo_lock(text: str) -> dict[tuple[str, str, str | None], str | None]:
    packages: dict[tuple[str, str, str | None], str | None] = {}
    current: dict[str, str] | None = None

    def finish() -> None:
        if current is None:
            return
        if "name" not in current or "version" not in current:
            raise ValueError("Cargo.lock has an incomplete package entry")
        key = (current["name"], current["version"], current.get("source"))
        checksum = current.get("checksum")
        if checksum is not None and not HEX_64.fullmatch(checksum):
            raise ValueError(f"Cargo.lock has an invalid checksum for {key[0]} {key[1]}")
        if key in packages:
            raise ValueError(f"Cargo.lock repeats package {key}")
        packages[key] = checksum

    for raw_line in text.splitlines():
        line = raw_line.strip()
        if line == "[[package]]":
            finish()
            current = {}
            continue
        if current is None:
            continue
        match = LOCK_VALUE.fullmatch(line)
        if match and match.group(1) in {"name", "version", "source", "checksum"}:
            current[match.group(1)] = json.loads(match.group(2))
    finish()
    return packages


def resolve_license(
    declared: str | None,
    choices: dict[str, str],
) -> tuple[str, str]:
    if not declared:
        raise ValueError("component has no declared license")
    normalized = declared.replace("/", " OR ")
    ambiguous = " OR " in normalized
    if ambiguous and declared not in choices:
        raise ValueError(f"license choice is missing for {declared}")
    selected = choices.get(declared, normalized)
    if not selected or "/" in selected or " OR " in selected:
        raise ValueError(f"license choice is not a single concluded branch for {declared}")
    return normalized, selected


def render_json(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False) + "\n").encode()


def check_outputs(expected: dict[Path, bytes]) -> None:
    stale = [str(path) for path, content in expected.items() if not path.is_file() or path.read_bytes() != content]
    if stale:
        raise ValueError("generated supply-chain output is stale: " + ", ".join(stale))


def write_outputs(expected: dict[Path, bytes]) -> None:
    for path, content in expected.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        temporary = path.with_suffix(path.suffix + ".tmp")
        temporary.write_bytes(content)
        temporary.replace(path)


def _cargo_license_files(repo_root: Path, package: dict[str, Any]) -> list[dict[str, Any]]:
    manifest = Path(package["manifest_path"])
    candidates: list[Path] = []
    declared_file = package.get("license_file")
    if declared_file:
        candidates.append(Path(declared_file))
    package_root = manifest.parent
    if package_root.is_dir():
        candidates.extend(
            child
            for child in package_root.iterdir()
            if child.is_file() and child.name.lower().startswith(LICENSE_FILE_PREFIXES)
        )
    if package.get("source") is None and repo_root.is_dir():
        candidates.extend(
            child
            for child in repo_root.iterdir()
            if child.is_file() and child.name.lower().startswith(LICENSE_FILE_PREFIXES)
        )
    unique = sorted({path.resolve() for path in candidates if path.is_file()}, key=lambda item: item.name.lower())
    return [file_record(path) for path in unique]


def _cargo_ref(name: str, version: str) -> str:
    return f"pkg:cargo/{quote(name, safe='')}@{quote(version, safe='.+-_')}"


def _license_entries(declared: str, selected: str) -> list[dict[str, Any]]:
    entries = [{"expression": declared, "acknowledgement": "declared"}]
    if selected != declared:
        entries.append({"expression": selected, "acknowledgement": "concluded"})
    return entries


def _manual_license_entries(license_data: dict[str, str]) -> list[dict[str, Any]]:
    if "expression" in license_data:
        return [{"expression": license_data["expression"], "acknowledgement": "declared"}]
    if "declared_expression" in license_data and "selected_expression" in license_data:
        return [
            {"expression": license_data["declared_expression"], "acknowledgement": "declared"},
            {"expression": license_data["selected_expression"], "acknowledgement": "concluded"},
        ]
    if "name" in license_data:
        value: dict[str, Any] = {"name": license_data["name"], "acknowledgement": "declared"}
        if license_data.get("url"):
            value["url"] = license_data["url"]
        return [{"license": value}]
    raise ValueError(f"manual component has an invalid license object: {license_data}")


def _manual_component(repo_root: Path, source: dict[str, Any]) -> tuple[dict[str, Any], dict[str, Any]]:
    component = dict(source)
    asset_records = []
    asset_bytes = b""
    for relative in component.pop("asset_paths", []):
        path = repo_root / relative
        if not path.is_file():
            raise ValueError(f"manual component asset is missing: {relative}")
        asset_records.append(file_record(path, relative))
        asset_bytes += path.read_bytes()
    for marker in component.pop("asset_markers", []):
        if marker.encode() not in asset_bytes:
            raise ValueError(f"manual component asset marker is missing: {marker}")
    license_records = []
    for relative in component.pop("license_paths", []):
        path = repo_root / relative
        if not path.is_file():
            raise ValueError(f"manual component license is missing: {relative}")
        license_records.append(file_record(path, relative))
    component["assets"] = sorted(asset_records, key=lambda item: item["path"])
    component["license_files"] = sorted(license_records, key=lambda item: item["path"])

    sbom = {
        "bom-ref": component["bom_ref"],
        "type": component["type"],
        "name": component["name"],
        "version": component["version"],
        "licenses": _manual_license_entries(component["license"]),
        "properties": [
            {"name": "koett:group", "value": component["group"]},
            {"name": "koett:notice-state", "value": component["notice_state"]},
        ],
    }
    if component["bom_ref"].startswith("pkg:"):
        sbom["purl"] = component["bom_ref"]
    if component.get("hashes"):
        sbom["hashes"] = component["hashes"]
    elif len(asset_records) == 1:
        sbom["hashes"] = [{"alg": "SHA-256", "content": asset_records[0]["sha256"]}]
    if component.get("source_url"):
        sbom["externalReferences"] = [{"type": "distribution", "url": component["source_url"]}]
    if component.get("status"):
        sbom["properties"].append({"name": "koett:status", "value": component["status"]})
    component["sbom_ref"] = component.pop("bom_ref")
    return component, sbom


def _input_records(repo_root: Path, policy_path: Path) -> list[dict[str, Any]]:
    paths = [
        "CrossPlatform/Cargo.lock",
        "CrossPlatform/Cargo.toml",
        "Package.resolved",
        "Package.swift",
        "LICENSE",
        "CrossPlatform/THIRD_PARTY_NOTICES.md",
        "CrossPlatform/evidence/blocked-604ed15/control.json",
        "Sources/Koett/Resources/AssistantRenderer/THIRD-PARTY-NOTICES.md",
    ]
    records = [file_record(policy_path, str(policy_path.relative_to(repo_root)))]
    for relative in paths:
        path = repo_root / relative
        if path.is_file():
            records.append(file_record(path, relative))
    return sorted(records, key=lambda item: item["path"])


def _validate_policy_inputs(repo_root: Path, policy: dict[str, Any]) -> None:
    if policy.get("schema") != 1:
        raise ValueError("inventory policy schema must be 1")
    for relative, markers in policy.get("required_markers", {}).items():
        path = repo_root / relative
        if not path.is_file():
            raise ValueError(f"required inventory input is missing: {relative}")
        content = path.read_bytes()
        for marker in markers:
            if marker.encode() not in content:
                raise ValueError(f"required inventory marker is missing from {relative}: {marker}")


def build_documents(
    repo_root: Path,
    metadata: dict[str, Any],
    lock_packages: dict[tuple[str, str, str | None], str | None],
    policy: dict[str, Any],
) -> Bundle:
    choices = policy.get("license_choices", {})
    packages = sorted(metadata["packages"], key=lambda item: (item["name"], item["version"], item["id"]))
    root_id = metadata["resolve"]["root"]
    rust_components = []
    sbom_by_id: dict[str, dict[str, Any]] = {}
    gaps = []
    choices_used = []

    for package in packages:
        declared, selected = resolve_license(package.get("license"), choices)
        key = (package["name"], package["version"], package.get("source"))
        if key not in lock_packages:
            raise ValueError(f"Cargo.lock is missing {package['name']} {package['version']}")
        checksum = lock_packages[key]
        if (package.get("source") or "").startswith("registry+") and checksum is None:
            raise ValueError(f"registry package has no Cargo.lock checksum: {package['name']} {package['version']}")
        license_files = _cargo_license_files(repo_root, package)
        notice_state = "project-license" if package["id"] == root_id else "missing"
        if package["id"] != root_id:
            reason = "license-text-not-bundled" if license_files else "source-license-text-not-found"
            gaps.append({
                "component": _cargo_ref(package["name"], package["version"]),
                "group": "rust-package",
                "reason": reason,
            })
        component = {
            "group": "rust-package",
            "name": package["name"],
            "version": package["version"],
            "source": package.get("source"),
            "repository": package.get("repository"),
            "declared_license": package["license"],
            "normalized_declared_license": declared,
            "selected_license": selected,
            "cargo_checksum_sha256": checksum,
            "license_files": license_files,
            "notice_state": notice_state,
            "sbom_ref": _cargo_ref(package["name"], package["version"]),
        }
        rust_components.append(component)
        if declared != selected:
            choices_used.append({
                "component": component["sbom_ref"],
                "declared": package["license"],
                "normalized_declared": declared,
                "selected": selected,
            })
        sbom_component: dict[str, Any] = {
            "bom-ref": component["sbom_ref"],
            "type": "library" if package["id"] != root_id else "application",
            "name": package["name"],
            "version": package["version"],
            "purl": component["sbom_ref"],
            "licenses": _license_entries(declared, selected),
            "properties": [
                {"name": "koett:group", "value": "rust-package"},
                {"name": "koett:notice-state", "value": notice_state},
            ],
        }
        if checksum:
            sbom_component["hashes"] = [{"alg": "SHA-256", "content": checksum}]
        if package.get("repository"):
            sbom_component["externalReferences"] = [{"type": "vcs", "url": package["repository"]}]
        sbom_by_id[package["id"]] = sbom_component

    manual_inventory = []
    manual_sbom = []
    for source in sorted(policy.get("manual_components", []), key=lambda item: item["bom_ref"]):
        component, sbom_component = _manual_component(repo_root, source)
        manual_inventory.append(component)
        manual_sbom.append(sbom_component)
        if component["notice_state"] != "bundled":
            gaps.append({
                "component": component["sbom_ref"],
                "group": component["group"],
                "reason": "license-or-attribution-not-bundled",
            })
        license_data = component["license"]
        if "declared_expression" in license_data:
            choices_used.append({
                "component": component["sbom_ref"],
                "declared": license_data["declared_expression"],
                "normalized_declared": license_data["declared_expression"],
                "selected": license_data["selected_expression"],
            })

    id_to_ref = {package["id"]: _cargo_ref(package["name"], package["version"]) for package in packages}
    dependency_map: dict[str, set[str]] = {ref: set() for ref in id_to_ref.values()}
    for node in metadata["resolve"]["nodes"]:
        dependency_map[id_to_ref[node["id"]]].update(id_to_ref[item] for item in node["dependencies"])
    root_ref = id_to_ref[root_id]
    dependency_map[root_ref].update(component["bom-ref"] for component in manual_sbom)
    for component in manual_sbom:
        dependency_map.setdefault(component["bom-ref"], set())
    dependencies = [
        {"ref": ref, "dependsOn": sorted(depends_on)}
        for ref, depends_on in sorted(dependency_map.items())
    ]

    all_components = rust_components + manual_inventory
    groups = {group: sum(item["group"] == group for item in all_components) for group in {
        "rust-package", "swift-package", "bundled-javascript", "native-library", "model"
    }}
    gaps.sort(key=lambda item: (item["group"], item["component"], item["reason"]))
    choices_used.sort(key=lambda item: item["component"])
    policy_path = repo_root / "CrossPlatform" / "supply-chain" / "inventory-policy.json"
    inventory_document = {
        "schema": 1,
        "status": policy["status"],
        "inputs": _input_records(repo_root, policy_path) if policy_path.is_file() else [],
        "summary": {
            "components": len(all_components),
            "rust_packages": groups["rust-package"],
            "swift_packages": groups["swift-package"],
            "bundled_javascript": groups["bundled-javascript"],
            "native_libraries": groups["native-library"],
            "models": groups["model"],
            "notice_gaps": len(gaps),
            "blocked_control_components": sum(item.get("status") == "blocked-control-only" for item in all_components),
        },
        "license_choices": choices_used,
        "components": sorted(all_components, key=lambda item: (item["group"], item["name"], item["version"])),
        "dependency_edges": dependencies,
    }
    primary = dict(sbom_by_id[root_id])
    primary["properties"] = primary.get("properties", []) + [
        {"name": "koett:inventory-status", "value": policy["status"]}
    ]
    sbom_document = {
        "$schema": "http://cyclonedx.org/schema/bom-1.7.schema.json",
        "bomFormat": "CycloneDX",
        "specVersion": "1.7",
        "version": 1,
        "metadata": {
            "lifecycles": [{"phase": "pre-build"}],
            "component": primary,
        },
        "components": sorted(
            [value for package_id, value in sbom_by_id.items() if package_id != root_id] + manual_sbom,
            key=lambda item: item["bom-ref"],
        ),
        "dependencies": dependencies,
        "compositions": [{"aggregate": "incomplete", "assemblies": [root_ref]}],
    }
    gap_document = {
        "schema": 1,
        "status": "known-gaps-block-release-compliance",
        "summary": {"gaps": len(gaps)},
        "gaps": gaps,
    }
    return Bundle(inventory_document, sbom_document, gap_document)


def _cargo_metadata() -> dict[str, Any]:
    result = subprocess.run(
        ["cargo", "+1.98.0", "metadata", "--locked", "--format-version", "1"],
        cwd=CROSS_PLATFORM,
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(result.stdout)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="fail if checked-in output is stale")
    args = parser.parse_args()
    policy = json.loads(POLICY_PATH.read_text(encoding="utf-8"))
    _validate_policy_inputs(REPOSITORY_ROOT, policy)
    lock_packages = parse_cargo_lock((CROSS_PLATFORM / "Cargo.lock").read_text(encoding="utf-8"))
    documents = build_documents(REPOSITORY_ROOT, _cargo_metadata(), lock_packages, policy)
    expected = {
        OUTPUTS["inventory"]: render_json(documents.inventory),
        OUTPUTS["sbom"]: render_json(documents.sbom),
        OUTPUTS["gaps"]: render_json(documents.gaps),
    }
    if args.check:
        check_outputs(expected)
    else:
        write_outputs(expected)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
