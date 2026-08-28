#!/usr/bin/env python3
"""Tests for Koett's dependency, license, and draft SBOM inventory."""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.dont_write_bytecode = True

SCRIPT = Path(__file__).with_name("generate_supply_chain_inventory.py")
SPEC = importlib.util.spec_from_file_location("generate_supply_chain_inventory", SCRIPT)
assert SPEC and SPEC.loader
inventory = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(inventory)


class SupplyChainInventoryTests(unittest.TestCase):
    def test_cargo_lock_parser_keeps_registry_checksums(self) -> None:
        packages = inventory.parse_cargo_lock(
            """
version = 4

[[package]]
name = "demo"
version = "1.2.3"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

[[package]]
name = "local"
version = "0.1.0"
"""
        )

        self.assertEqual(
            packages[(
                "demo",
                "1.2.3",
                "registry+https://github.com/rust-lang/crates.io-index",
            )],
            "a" * 64,
        )
        self.assertIsNone(packages[("local", "0.1.0", None)])

    def test_ambiguous_license_without_choice_fails_closed(self) -> None:
        with self.assertRaisesRegex(ValueError, "license choice"):
            inventory.resolve_license("MIT OR Apache-2.0", {})

    def test_legacy_slash_license_is_normalized_and_selected(self) -> None:
        declared, selected = inventory.resolve_license(
            "MIT/Apache-2.0",
            {"MIT/Apache-2.0": "MIT"},
        )

        self.assertEqual(declared, "MIT OR Apache-2.0")
        self.assertEqual(selected, "MIT")

    def test_cargo_purl_keeps_semver_build_metadata_separator(self) -> None:
        self.assertEqual(
            inventory._cargo_ref("demo", "1.2.3+metadata"),
            "pkg:cargo/demo@1.2.3+metadata",
        )

    def test_build_documents_include_all_component_groups_and_gaps(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            license_path = root / "LICENSE.demo"
            asset_path = root / "demo.js"
            style_path = root / "demo.css"
            license_path.write_text("demo license\n", encoding="utf-8")
            asset_path.write_text("demo asset\n", encoding="utf-8")
            style_path.write_text("demo style\n", encoding="utf-8")
            metadata = {
                "packages": [
                    {
                        "name": "koett-engine",
                        "version": "0.1.0",
                        "id": "path+file:///repo#koett-engine@0.1.0",
                        "license": "Apache-2.0",
                        "license_file": None,
                        "manifest_path": str(root / "Cargo.toml"),
                        "repository": None,
                        "source": None,
                    },
                    {
                        "name": "demo-crate",
                        "version": "1.2.3",
                        "id": "registry+index#demo-crate@1.2.3",
                        "license": "MIT OR Apache-2.0",
                        "license_file": None,
                        "manifest_path": str(root / "registry" / "demo" / "Cargo.toml"),
                        "repository": "https://example.test/demo",
                        "source": "registry+https://github.com/rust-lang/crates.io-index",
                    },
                ],
                "resolve": {
                    "root": "path+file:///repo#koett-engine@0.1.0",
                    "nodes": [
                        {
                            "id": "path+file:///repo#koett-engine@0.1.0",
                            "dependencies": ["registry+index#demo-crate@1.2.3"],
                        },
                        {
                            "id": "registry+index#demo-crate@1.2.3",
                            "dependencies": [],
                        },
                    ],
                },
            }
            policy = {
                "schema": 1,
                "status": "draft-source-inventory-not-release-compliance",
                "license_choices": {"MIT OR Apache-2.0": "MIT"},
                "manual_components": [
                    {
                        "bom_ref": "pkg:npm/demo-js@1.0.0",
                        "type": "library",
                        "group": "bundled-javascript",
                        "name": "demo-js",
                        "version": "1.0.0",
                        "license": {"expression": "MIT"},
                        "asset_paths": ["demo.js", "demo.css"],
                        "license_paths": ["LICENSE.demo"],
                        "notice_state": "bundled",
                    },
                    {
                        "bom_ref": "pkg:huggingface/example/model@abc",
                        "type": "machine-learning-model",
                        "group": "model",
                        "name": "example/model",
                        "version": "abc",
                        "license": {"expression": "CC-BY-4.0"},
                        "notice_state": "missing",
                    },
                ],
            }
            lock_packages = {
                (
                    "demo-crate",
                    "1.2.3",
                    "registry+https://github.com/rust-lang/crates.io-index",
                ): "b" * 64,
                ("koett-engine", "0.1.0", None): None,
            }

            first = inventory.build_documents(root, metadata, lock_packages, policy)
            second = inventory.build_documents(root, metadata, lock_packages, policy)

        self.assertEqual(
            inventory.render_json(first.inventory),
            inventory.render_json(second.inventory),
        )
        self.assertEqual(first.inventory["summary"]["rust_packages"], 2)
        self.assertEqual(first.inventory["summary"]["bundled_javascript"], 1)
        self.assertEqual(first.inventory["summary"]["models"], 1)
        self.assertEqual(len(first.gaps["gaps"]), 2)
        self.assertEqual(first.sbom["specVersion"], "1.7")
        refs = {component["bom-ref"] for component in first.sbom["components"]}
        self.assertIn("pkg:cargo/demo-crate@1.2.3", refs)
        self.assertIn("pkg:huggingface/example/model@abc", refs)
        demo = next(
            component
            for component in first.inventory["components"]
            if component["sbom_ref"] == "pkg:npm/demo-js@1.0.0"
        )
        self.assertEqual(
            [asset["path"] for asset in demo["assets"]],
            ["demo.css", "demo.js"],
        )

    def test_check_outputs_rejects_stale_file(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "output.json"
            output.write_text("{}\n", encoding="utf-8")

            with self.assertRaisesRegex(ValueError, "stale"):
                inventory.check_outputs({output: b'{"schema": 1}\n'})


if __name__ == "__main__":
    unittest.main()
