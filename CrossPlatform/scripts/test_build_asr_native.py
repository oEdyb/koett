import unittest

from build_asr_native import (
    OPTIONS,
    configure_command,
    native_options,
    remove_disabled_tts_stubs,
)


class AsrBuildTests(unittest.TestCase):
    def test_windows_selects_msvc_platform_for_the_static_onnx_archive(self):
        command = configure_command('source', 'build', 'x86_64-pc-windows-msvc',
                                    'Visual Studio 18 2026')
        self.assertEqual(command[:9], ['cmake', '-S', 'source', '-B', 'build', '-G',
                                     'Visual Studio 18 2026', '-A', 'x64'])
        self.assertIn('-DSHERPA_ONNX_ENABLE_TTS=OFF', command)
        self.assertNotIn('Ninja', command)

    def test_default_generators_preserve_linux_and_windows_toolchains(self):
        linux = configure_command('source', 'build', 'aarch64-unknown-linux-gnu')
        self.assertEqual(linux[5:7], ['-G', 'Ninja'])
        self.assertIn('-DCMAKE_CXX_FLAGS=-D_GLIBCXX_USE_CXX11_ABI=0', linux)
        windows = configure_command('source', 'build', 'x86_64-pc-windows-msvc')
        self.assertEqual(windows[5:7], ['-A', 'x64'])
        self.assertNotIn('-G', windows)

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
