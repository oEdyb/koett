import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('windows_local_build', Path(__file__).with_name('windows_local_build.py'))
build = importlib.util.module_from_spec(spec)
spec.loader.exec_module(build)


class LocalWindowsBuildTests(unittest.TestCase):
    def setUp(self):
        root = Path.home() / '.cache/koett-windows/test-temp'
        root.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(dir=root)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.sdk = self.root / 'sdk'
        self.native = self.root / 'native/lib'
        for name in ('crt/lib/x86_64', 'sdk/lib/um/x86_64', 'sdk/lib/ucrt/x86_64',
                     'crt/include', 'sdk/include/ucrt', 'sdk/include/um', 'sdk/include/shared'):
            (self.sdk / name).mkdir(parents=True)
        self.native.mkdir(parents=True)
        for name in ('koett-asr-only.txt', 'koett-context.txt', 'core.lib'):
            (self.native / name).write_text('fixture')
        (self.native.parent / 'build-receipt.json').write_text('{}')

    def test_cross_build_does_not_inherit_host_sqlite_or_documentation_mode(self):
        stale = {'DOCS_RS': '1', 'LIBSQLITE3_SYS_USE_PKG_CONFIG': '1', 'PKG_CONFIG_ALLOW_CROSS': '1',
                 'RUSTFLAGS': '-L native=/wrong', 'CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS': '-L native=/wrong'}
        with patch.dict(os.environ, stale), patch.object(build.subprocess, 'check_output', return_value=''):
            env = build.environment(self.sdk, self.native, self.root / 'cache')
        for key in stale:
            self.assertNotIn(key, env)
        self.assertEqual(env['CARGO_BUILD_JOBS'], '4')
        self.assertEqual(env['SHERPA_ONNX_LIB_DIR'], str(self.native))
        self.assertIn('target-feature=+crt-static', env['CARGO_ENCODED_RUSTFLAGS'])
        self.assertIn(str(self.sdk / 'sdk/lib/um/x86_64'), env['CARGO_ENCODED_RUSTFLAGS'])
        self.assertTrue(Path(env['TMPDIR']).is_dir())

    def test_coff_library_case_alias_uses_the_existing_sdk_library(self):
        library = self.sdk / 'sdk/lib/um/x86_64/pathcch.lib'
        library.write_text('fixture')
        with patch.object(build.subprocess, 'check_output', return_value=' /DEFAULTLIB:PathCch.lib /DEFAULTLIB:"missing.lib"'):
            build.environment(self.sdk, self.native, self.root / 'cache')
        alias = library.with_name('PathCch.lib')
        self.assertTrue(alias.is_symlink())
        self.assertEqual(alias.resolve(), library)
        saved = json.loads((self.sdk / '.koett-coff-aliases.json').read_text())
        self.assertEqual(saved['aliases'], [str(alias)])
        with patch.object(build.subprocess, 'check_output', side_effect=AssertionError('unchanged native archive rescanned')):
            build.environment(self.sdk, self.native, self.root / 'cache')

    def test_missing_native_context_proof_stops_before_cargo(self):
        (self.native / 'koett-context.txt').unlink()
        with self.assertRaisesRegex(ValueError, 'koett-context.txt'):
            build.environment(self.sdk, self.native, self.root / 'cache')

    def test_ram_filesystem_is_rejected(self):
        with self.assertRaisesRegex(ValueError, '/tmp'):
            build.environment(self.sdk, self.native, Path('/tmp/koett-build'))
        with patch.dict(os.environ, {'CARGO_TARGET_DIR': '/tmp/koett-target'}), patch.object(build.subprocess, 'check_output', return_value=''):
            with self.assertRaisesRegex(ValueError, 'CARGO_TARGET_DIR'):
                build.environment(self.sdk, self.native, self.root / 'cache')


if __name__ == '__main__':
    unittest.main()
