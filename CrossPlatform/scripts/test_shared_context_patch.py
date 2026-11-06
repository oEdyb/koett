import hashlib
from pathlib import Path
import unittest

from shared_context_patch import ORIGINAL_SHA256, PATCHED_SHA256, apply_shared_context


class SharedContextPatchTests(unittest.TestCase):
    def test_exact_pinned_source_produces_reviewed_patch_and_is_idempotent(self):
        source = Path(__file__).with_name('fixtures').joinpath('offline-recognizer-transducer-nemo-impl.h').read_text()
        self.assertEqual(hashlib.sha256(source.encode()).hexdigest(), ORIGINAL_SHA256)
        patched = apply_shared_context(source)
        self.assertEqual(hashlib.sha256(patched.encode()).hexdigest(), PATCHED_SHA256)
        self.assertEqual(apply_shared_context(patched), patched)
        self.assertIn('config_.decoding_method == "modified_beam_search"', patched)

    def test_changed_source_is_rejected_before_patching(self):
        with self.assertRaisesRegex(ValueError, 'unrecognized NeMo source'):
            apply_shared_context('unreviewed upstream content')


if __name__ == '__main__':
    unittest.main()
