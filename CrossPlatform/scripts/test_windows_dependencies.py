#!/usr/bin/env python3
import struct
import tempfile
import unittest
from pathlib import Path
from check_windows_dependencies import imports, verify


def executable(name, delayed=False):
    data=bytearray(2048)
    data[:2]=b'MZ';struct.pack_into('<I',data,60,128);data[128:132]=b'PE\0\0'
    struct.pack_into('<H',data,134,1);struct.pack_into('<H',data,148,240)
    optional=152;struct.pack_into('<H',data,optional,0x20b)
    struct.pack_into('<Q',data,optional+24,0x140000000)
    struct.pack_into('<IIII',data,optional+240+8,1536,0x1000,1536,512)
    directory=13 if delayed else 1
    struct.pack_into('<II',data,optional+112+directory*8,0x1000,64 if delayed else 40)
    if delayed:struct.pack_into('<II',data,512,1,0x1080)
    else:struct.pack_into('<I',data,524,0x1080)
    encoded=name.encode()+b'\0';data[640:640+len(encoded)]=encoded
    return data


class DependencyTests(unittest.TestCase):
    def test_reads_regular_and_delayed_imports(self):
        for delayed in [False,True]:
            self.assertEqual(imports(executable('VCRUNTIME140_1.dll',delayed)),['vcruntime140_1.dll'])
    def test_rejects_missing_clean_machine_prerequisite(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'Koett.exe';path.write_bytes(executable('VCRUNTIME140_1.dll'))
            with self.assertRaisesRegex(ValueError,'non-system DLLs: vcruntime140_1.dll'):verify(path)
    def test_accepts_windows_system_import(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'Koett.exe';path.write_bytes(executable('KERNEL32.dll'))
            self.assertTrue(verify(path)['system_only'])
    def test_accepts_windows_theme_import(self):
        for delayed in [False,True]:
            with self.subTest(delayed=delayed), tempfile.TemporaryDirectory() as directory:
                path=Path(directory)/'Koett.exe';path.write_bytes(executable('UxTheme.dll',delayed))
                self.assertTrue(verify(path)['system_only'])
    def test_rejects_unknown_imports(self):
        for name in ['unknown.dll','uxtheme_extra.dll','VCRUNTIME140_1.dll']:
            for delayed in [False,True]:
                with self.subTest(name=name,delayed=delayed), tempfile.TemporaryDirectory() as directory:
                    path=Path(directory)/'Koett.exe';path.write_bytes(executable(name,delayed))
                    with self.assertRaisesRegex(ValueError,'non-system DLLs: '+name.lower()):verify(path)
    def test_rejects_invalid_executable(self):
        with self.assertRaisesRegex(ValueError,'not a PE'):imports(b'bad')


if __name__=='__main__':unittest.main()
