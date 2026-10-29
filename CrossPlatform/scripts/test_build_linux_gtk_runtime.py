import json
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import build_linux_gtk_runtime as build


class RuntimeBuildInputs(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def archive_and_patch(self):
        original = self.root / "original"
        original.mkdir()
        (original / "child.c").write_text("before\n")
        (original / "other.c").write_text("unchanged\n")
        archive = self.root / "source.tar.xz"
        with tarfile.open(archive, "w:xz") as bundle:
            bundle.add(original, arcname=f"gtk-{build.VERSION}")
        delta = self.root / "upstream.patch"
        delta.write_text("--- a/child.c\n+++ b/child.c\n@@ -1 +1 @@\n-before\n+after\n")
        return archive, delta

    def test_cached_unrelated_source_edit_is_rejected_and_preserved(self):
        archive, delta = self.archive_and_patch()
        first = build.verify_source(self.root, archive, delta)
        self.assertEqual(build.verify_source(self.root, archive, delta), first)
        changed = self.root / f"gtk-{build.VERSION}/other.c"
        changed.write_text("local modification\n")
        with self.assertRaisesRegex(ValueError, "other.c"):
            build.verify_source(self.root, archive, delta)
        self.assertEqual(changed.read_text(), "local modification\n")

    def test_untracked_source_input_is_rejected(self):
        archive, delta = self.archive_and_patch()
        build.verify_source(self.root, archive, delta)
        (self.root / f"gtk-{build.VERSION}/unexpected.py").write_text("extra input")
        with self.assertRaisesRegex(ValueError, "unexpected.py"):
            build.verify_source(self.root, archive, delta)

    def build_metadata(self, prefix, source, buildtype="release"):
        directory = self.root / "build"
        information = directory / "meson-info"
        information.mkdir(parents=True, exist_ok=True)
        options = [{"name": "prefix", "value": str(prefix)},
                   {"name": "buildtype", "value": buildtype}]
        (information / "intro-buildoptions.json").write_text(json.dumps(options))
        (information / "intro-buildsystem_files.json").write_text(json.dumps([str(source / "meson.build")]))
        return directory

    def test_reused_protocols_build_with_other_prefix_is_rejected(self):
        source = self.root / "source"
        directory = self.build_metadata(self.root / "outside-prefix", source)
        with self.assertRaisesRegex(ValueError, "prefix"):
            build.verify_build(directory, source, self.root / "private-prefix")

    def test_reused_debug_build_cannot_claim_release(self):
        source, prefix = self.root / "source", self.root / "private-prefix"
        directory = self.build_metadata(prefix, source, "debug")
        with self.assertRaisesRegex(ValueError, "buildtype"):
            build.verify_build(directory, source, prefix, {"buildtype": "release"})

    def test_reused_build_from_different_source_is_rejected(self):
        prefix = self.root / "private-prefix"
        directory = self.build_metadata(prefix, self.root / "unrelated-source")
        with self.assertRaisesRegex(ValueError, "source directory"):
            build.verify_build(directory, self.root / "expected-source", prefix)

    def test_corrupt_cached_download_is_not_executed_or_replaced(self):
        cached = self.root / "archive"
        cached.write_bytes(b"wrong content")
        with (
            patch.object(build, "urlopen", side_effect=AssertionError("network not expected")),
            self.assertRaisesRegex(ValueError, "checksum mismatch"),
        ):
            build.download("https://example.invalid/source", cached, "0" * 64)
        self.assertEqual(cached.read_bytes(), b"wrong content")


if __name__ == "__main__":
    unittest.main()
