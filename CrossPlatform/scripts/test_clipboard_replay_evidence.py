import json
import unittest
from pathlib import Path


class ClipboardReplayEvidence(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        path = Path(__file__).resolve().parents[1] / "evidence/sketch-repeat-replay-20261104/receipt.json"
        cls.receipt = json.loads(path.read_text())
        cls.intent = cls.receipt["intent"]

    def test_reads_do_not_distinguish_success_from_canceled_paste(self):
        accepted = self.intent["chrome_paste"]
        canceled = self.intent["canceled_key_plus_background_reader"]
        self.assertEqual(
            [r["mime"] for r in accepted["reads"]],
            [r["mime"] for r in canceled["reads"]],
        )
        self.assertTrue(accepted["pastes"])
        self.assertEqual(canceled["pastes"], [])
        self.assertTrue(canceled["signals"][0]["trusted"])
        self.assertTrue(canceled["focused"])
        self.assertLess(canceled["first_read_after_key_ms"], 1000)

    def test_one_shot_replay_can_insert_after_a_canceled_key(self):
        replay = self.intent["false_replay"]
        self.assertTrue(replay["focused"])
        self.assertTrue(replay["owned"])
        self.assertTrue(replay["original_trusted_key"]["trusted"])
        self.assertEqual(replay["original_paste_events"], 0)
        self.assertEqual(replay["after_replay_paste_events"], 1)
        self.assertEqual(replay["unexpected_images"], 1)

    def test_first_format_is_not_the_inserted_representation(self):
        replay = self.intent["first_format_inference_duplicate"]
        self.assertTrue(replay["first_requested_mime"].startswith("text/plain"))
        self.assertEqual(replay["original_images"], 1)
        self.assertEqual(replay["images_after_replay_image_leg"], 2)
        self.assertFalse(replay["replay_text_leg_run"])

    def test_window_match_does_not_protect_a_different_field(self):
        change = self.intent["focus_change"]
        self.assertTrue(change["same_browser_window"])
        self.assertFalse(change["original_target_still_focused"])
        self.assertEqual(change["unexpected_new_target_text"], "Koett repeat clipboard probe")

    def test_newer_copy_keeps_ownership_in_the_passive_probe(self):
        self.assertTrue(self.receipt["ownership"]["detached"])
        self.assertTrue(self.receipt["ownership"]["newer_copy_exact"])

    def test_static_formats_are_target_dependent(self):
        mixed = self.receipt["targets"]["mixed"]
        html = self.receipt["targets"]["html"]
        self.assertEqual((mixed["t3"]["images"], mixed["t3"]["text_occurrences"]), (1, 0))
        self.assertEqual((html["t3"]["images"], html["t3"]["text_occurrences"]), (0, 1))
        self.assertEqual((html["chrome"]["images"], html["chrome"]["text_occurrences"]), (1, 1))
        self.assertTrue(mixed["terminal"]["exact_text"])
        self.assertTrue(html["terminal"]["exact_text"])


if __name__ == "__main__":
    unittest.main()
