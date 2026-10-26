import unittest

from build_asr_native import OPTIONS, native_options, remove_disabled_tts_stubs


class AsrBuildTests(unittest.TestCase):
    def test_removes_only_disabled_tts_implementation_with_nested_guards(self):
        original = ("before\n#if SHERPA_ONNX_ENABLE_TTS == 1\n"
                    "struct SherpaOnnxOfflineTts {};\n#if __OHOS__\n"
                    "enabled_one\n#else\nenabled_two\n#endif\n"
                    "#else\ndisabled_tts_stub\n"
                    "#endif  // SHERPA_ONNX_ENABLE_TTS == 1\nafter\n")
        result = remove_disabled_tts_stubs(original)
        self.assertIn("enabled_one\n#else\nenabled_two", result)
        self.assertNotIn("disabled_tts_stub", result)
        self.assertTrue(result.startswith("before\n"))
        self.assertTrue(result.endswith("after\n"))

    def test_changed_source_fails_instead_of_patching_an_unknown_region(self):
        with self.assertRaises(ValueError):
            remove_disabled_tts_stubs("unrelated source")

    def test_missing_disabled_branch_is_an_error(self):
        with self.assertRaisesRegex(ValueError, "disabled branch"):
            remove_disabled_tts_stubs(
                "#if SHERPA_ONNX_ENABLE_TTS == 1\n"
                "struct SherpaOnnxOfflineTts {};\n#endif\n")

    def test_tts_and_unneeded_components_are_disabled(self):
        for component in ["TTS", "SPEAKER_DIARIZATION", "PORTAUDIO", "WEBSOCKET", "BINARY"]:
            self.assertEqual(OPTIONS[f"SHERPA_ONNX_ENABLE_{component}"], "OFF")

    def test_linux_uses_the_prebuilt_onnx_archive_cpp_abi(self):
        self.assertEqual(native_options("x86_64-unknown-linux-gnu")["CMAKE_CXX_FLAGS"],
                         "-D_GLIBCXX_USE_CXX11_ABI=0")
        self.assertNotIn("CMAKE_CXX_FLAGS", native_options("x86_64-pc-windows-msvc"))


if __name__ == "__main__":
    unittest.main()
