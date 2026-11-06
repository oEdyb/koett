#!/usr/bin/env python3
"""Cache trimming removes only what has a replacement."""

from __future__ import annotations

import unittest

import ci_trim_caches as trim


class TrimTests(unittest.TestCase):
    def test_families_ignore_only_what_changes_between_versions(self) -> None:
        lock, sha = "a" * 64, "b" * 40
        self.assertEqual(trim.family(f"cargo-v2-linux-x86_64-{lock}"), "cargo-v2-linux-x86_64")
        self.assertEqual(trim.family("apt-v1-arm64-202611"), "apt-v1-arm64")
        self.assertEqual(trim.family(f"cargo-v1-msrv-{lock}-{sha}"), "cargo-v1-msrv")
        self.assertNotEqual(trim.family(f"native-v1-ubuntu-22.04-{lock}"), trim.family(f"native-v1-ubuntu-22.04-arm-{lock}"))

    def test_a_replaced_cache_goes_and_the_current_one_stays(self) -> None:
        old, new = "cargo-v2-msrv-" + "a" * 64, "cargo-v2-msrv-" + "b" * 64
        caches = [{"key": old, "id": 1}, {"key": new, "id": 2}]
        self.assertEqual([c["id"] for c in trim.stale_caches(caches, [new])], [1])

    def test_nothing_goes_when_its_replacement_failed_to_save(self) -> None:
        old, new = "cargo-v2-msrv-" + "a" * 64, "cargo-v2-msrv-" + "b" * 64
        self.assertEqual(trim.stale_caches([{"key": old, "id": 1}], [new]), [])

    def test_another_familys_cache_is_left_alone(self) -> None:
        arm = "native-v1-ubuntu-22.04-arm-" + "a" * 64
        x86 = "native-v1-ubuntu-22.04-" + "b" * 64
        self.assertEqual(trim.stale_caches([{"key": arm, "id": 1}, {"key": x86, "id": 2}], [x86]), [])


if __name__ == "__main__":
    unittest.main()
