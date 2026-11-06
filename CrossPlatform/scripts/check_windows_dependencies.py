#!/usr/bin/env python3
"""Reject Windows executables that depend on unbundled non-system DLLs."""
import argparse
import json
from pathlib import Path
import struct

# SetWindowTheme and UI Automation are supplied by clean Windows 11 installs.
# https://learn.microsoft.com/en-us/windows/win32/api/uxtheme/nf-uxtheme-setwindowtheme
# https://learn.microsoft.com/en-us/windows/win32/api/uiautomationcoreapi/nf-uiautomationcoreapi-uiareturnrawelementprovider
SYSTEM = set('kernel32 shell32 combase secur32 crypt32 advapi32 user32 gdi32 d3d11 d2d1 dcomp dwrite comdlg32 dxgi ole32 mmdevapi oleaut32 bcryptprimitives ntdll ws2_32 dbghelp setupapi ucrtbase bcrypt propsys shlwapi comctl32 dwmapi winmm winhttp ncrypt normaliz rpcrt4 cfgmgr32 powrprof version uuid wininet userenv wtsapi32 uxtheme uiautomationcore'.split())


def imports(data):
    def number(fmt, offset):
        return struct.unpack_from(fmt, data, offset)[0]
    if data[:2] != b'MZ':
        raise ValueError('not a PE executable')
    pe = number('<I', 60)
    if data[pe:pe+4] != b'PE\0\0':
        raise ValueError('invalid PE signature')
    optional = pe + 24
    magic = number('<H', optional)
    if magic not in (0x10b, 0x20b):
        raise ValueError('unknown PE optional header')
    directories = optional + (112 if magic == 0x20b else 96)
    image_base = number('<Q' if magic == 0x20b else '<I', optional + (24 if magic == 0x20b else 28))
    sections = optional + number('<H', pe+20)
    def offset(rva):
        for index in range(number('<H', pe+6)):
            section = sections + 40*index
            size, address, raw_size, raw = struct.unpack_from('<IIII', data, section+8)
            if address <= rva < address+max(size, raw_size):
                position = raw+rva-address
                if position >= len(data):
                    break
                return position
        raise ValueError(f'unmapped PE RVA {rva:x}')
    result = set()
    for directory, stride, name_index in [(1,20,12),(13,32,4)]:
        rva, size = struct.unpack_from('<II', data, directories+directory*8)
        if not rva:
            continue
        table = offset(rva)
        for index in range(size//stride):
            entry = table+index*stride
            if not any(data[entry:entry+stride]):
                break
            name_rva = number('<I', entry+name_index)
            if directory == 13 and not number('<I', entry) & 1:
                name_rva -= image_base
            start = offset(name_rva)
            end = data.index(b'\0', start)
            result.add(data[start:end].decode('ascii').lower())
    return sorted(result)


def verify(path):
    dependencies = imports(Path(path).read_bytes())
    external = [name for name in dependencies if not name.startswith(('api-ms-win-', 'ext-ms-win-')) and name.removesuffix('.dll') not in SYSTEM]
    if external:
        raise ValueError('Windows executable requires non-system DLLs: '+', '.join(external))
    return {'executable': str(path), 'dependencies': dependencies, 'system_only': True}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('executable', type=Path)
    print(json.dumps(verify(parser.parse_args().executable), indent=2))
