import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from build_asr_native import (
    LIBRARIES,
    ONNX_CPP_ABI_SYMBOLS,
    OPTIONS,
    SOURCE_COMMIT,
    configure_command,
    native_options,
    remove_disabled_tts_stubs,
    stage_libraries,
    validate_onnx_cpp_abi,
)


class AsrBuildTests(unittest.TestCase):
    def test_onnx_string_abi_must_match_the_target_and_be_unambiguous(self):
        for target, expected in [('x86_64-unknown-linux-gnu', '0'),
                                 ('aarch64-unknown-linux-gnu', '1')]:
            with self.subTest(target=target):
                validate_onnx_cpp_abi(target, '0000 T ' + ONNX_CPP_ABI_SYMBOLS[expected])
                for symbols in ['', '0000 T ' + ONNX_CPP_ABI_SYMBOLS[str(1 - int(expected))],
                                '\n'.join('0000 T ' + s for s in ONNX_CPP_ABI_SYMBOLS.values())]:
                    with self.assertRaisesRegex(ValueError, 'ABI mismatch'):
                        validate_onnx_cpp_abi(target, symbols)

    def test_failed_abi_validation_revokes_a_previous_native_marker(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            build = root / 'build'
            build.mkdir()
            (root / 'lib').mkdir()
            marker = root / 'lib/koett-asr-only.txt'
            marker.write_bytes(f'koett-asr-only-v1\nsource={SOURCE_COMMIT}\ntarget=aarch64-unknown-linux-gnu\n'.encode())
            for name in LIBRARIES:
                (build / f'lib{name}.a').write_bytes(b'private archive fixture')
            wrong = '0000 T ' + ONNX_CPP_ABI_SYMBOLS['0']
            with (patch('build_asr_native.subprocess.check_output', return_value=wrong),
                  self.assertRaisesRegex(ValueError, 'ABI mismatch')):
                stage_libraries(build, root, 'aarch64-unknown-linux-gnu')
            self.assertFalse(marker.exists())

    def test_unreviewed_linux_target_has_no_implicit_abi(self):
        with self.assertRaisesRegex(ValueError, 'unreviewed Linux'):
            native_options('riscv64gc-unknown-linux-gnu')

    def test_windows_text_newlines_do_not_change_the_native_receipt_contract(self):
        def windows_write_text(path, data):
            with path.open('w', encoding='utf-8', newline='\r\n') as output:
                return output.write(data)

        target = 'x86_64-pc-windows-msvc'
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            build = root / 'build'
            build.mkdir()
            for name in LIBRARIES:
                (build / f'{name}.lib').write_bytes(b'private static archive fixture')
            with patch.object(Path, 'write_text', windows_write_text), contextlib.redirect_stdout(io.StringIO()):
                stage_libraries(build, root, target)
            marker = (root / 'lib/koett-asr-only.txt').read_bytes()
            self.assertEqual(marker, f'koett-asr-only-v1\nsource={SOURCE_COMMIT}\ntarget={target}\n'.encode())
            self.assertNotIn(b'\r', marker)
            self.assertEqual(json.loads((root / 'build-receipt.json').read_text())['target'], target)

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
        self.assertIn('-DCMAKE_CXX_FLAGS=-D_GLIBCXX_USE_CXX11_ABI=1', linux)
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
        self.assertEqual(native_options("aarch64-unknown-linux-gnu")["CMAKE_CXX_FLAGS"],
                         "-D_GLIBCXX_USE_CXX11_ABI=1")
        self.assertNotIn("CMAKE_CXX_FLAGS", native_options("x86_64-pc-windows-msvc"))


if __name__ == "__main__":
    unittest.main()
