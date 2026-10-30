#!/usr/bin/env python3
"""Report current-PC loader dependencies and optional setup without reading user data."""
import ctypes
import json
import os
import shutil
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parent
environment = dict(os.environ)
if (root / 'runtime/gtk').is_dir():
    environment['LD_LIBRARY_PATH'] = f'{root}/runtime/gtk/lib:{root}/runtime/asr/lib' + (':' + os.environ['LD_LIBRARY_PATH'] if os.environ.get('LD_LIBRARY_PATH') else '')
checks = {}
private = (root / 'runtime/gtk').is_dir()
required = ['libexec/koett'] + (['runtime/gtk/lib/libgtk-4.so.1'] if private else [])
for name in required:
    result = subprocess.run(['ldd', str(root / name)], env=environment, capture_output=True, text=True, check=False)
    checks[name] = {'available': result.returncode == 0 and 'not found' not in result.stdout, 'loader': result.stdout}
if not private:
    try:
        gtk = ctypes.CDLL('libgtk-4.so.1')
        version = tuple(getattr(gtk, 'gtk_get_' + part + '_version')() for part in ['major', 'minor', 'micro'])
        checks['gtk_minimum'] = {'available': version >= (4, 6, 0), 'version': version, 'minimum': '4.6.0', 'known_limitation': 'System GTK 4.24.1 needs upstream accessibility fix 21d3eb7; preflight cannot verify distro patches or desktop behavior.'}
    except OSError as error:
        checks['gtk_minimum'] = {'available': False, 'error': str(error), 'minimum': '4.6.0'}
optional = {
    'media': {'commands': [str(Path.home() / '.local/bin/yt-dlp'), '/usr/bin/ffmpeg'], 'setup': 'Install yt-dlp at ~/.local/bin/yt-dlp and FFmpeg at /usr/bin/ffmpeg; dictation works without media.'},
    'formatting': {'commands': ['llama-server'], 'setup': 'Install llama.cpp llama-server on PATH, then select/download the verified S1-mini model in Settings. Raw remains the default.'},
    'clipboard_wayland': {'commands': ['wl-copy'], 'setup': 'Install wl-clipboard for Wayland clipboard delivery.'},
    'typing_wayland': {'commands': ['wtype'], 'setup': 'Install wtype if supported by the compositor; Copy Last remains available.'},
    'ask_keys': {'commands': ['secret-tool'], 'setup': 'Install libsecret tools and enable a session Secret Service, then configure an optional Ask provider key in Settings.'},
}
for item in optional.values():
    item['resolved'] = {command: shutil.which(command, path=str(root / 'runtime/bin') + os.pathsep + os.environ.get('PATH', '')) for command in item['commands']}
ldconfig = shutil.which('ldconfig') or '/sbin/ldconfig'
webkit = subprocess.run([ldconfig, '-p'], capture_output=True, text=True, check=False) if Path(ldconfig).exists() else None
optional['rich_ask'] = {'available': webkit is not None and 'libwebkitgtk-6.0.so.4' in webkit.stdout, 'setup': 'Install the complete WebKitGTK 6 package for this distribution, including subprocess helpers and resources. Restart Koett. Plain Ask remains available without it; sandbox stays enabled.'}
print(json.dumps({'scope': 'current-PC private bundle' if private else 'system-runtime candidate; Python >= 3.10, GTK >= 4.6 and all ELF loader dependencies required', 'required': checks, 'optional': optional}, indent=2))
raise SystemExit(0 if all(item['available'] for item in checks.values()) else 1)
