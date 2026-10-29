#!/usr/bin/env python3
"""Report current-PC loader dependencies and optional setup without reading user data."""
import json
import os
import shutil
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parent
environment = {**os.environ, 'LD_LIBRARY_PATH': f'{root}/runtime/gtk/lib:{root}/runtime/asr/lib'}
checks = {}
for name in ['libexec/koett', 'runtime/gtk/lib/libgtk-4.so.1']:
    result = subprocess.run(['ldd', str(root / name)], env=environment, capture_output=True, text=True, check=False)
    checks[name] = {'available': result.returncode == 0 and 'not found' not in result.stdout, 'loader': result.stdout}
optional = {
    'media': {'commands': [str(Path.home() / '.local/bin/yt-dlp'), '/usr/bin/ffmpeg'], 'setup': 'Install yt-dlp at ~/.local/bin/yt-dlp and FFmpeg at /usr/bin/ffmpeg; dictation works without media.'},
    'formatting': {'commands': ['llama-server'], 'setup': 'Install llama.cpp llama-server on PATH, then select/download the verified S1-mini model in Settings. Raw remains the default.'},
    'clipboard_wayland': {'commands': ['wl-copy'], 'setup': 'Install wl-clipboard for Wayland clipboard delivery.'},
    'typing_wayland': {'commands': ['wtype'], 'setup': 'Install wtype if supported by the compositor; Copy Last remains available.'},
    'ask_keys': {'commands': ['secret-tool'], 'setup': 'Install libsecret tools and enable a session Secret Service, then configure an optional Ask provider key in Settings.'},
}
for item in optional.values():
    item['resolved'] = {command: shutil.which(command, path=str(root / 'runtime/bin') + os.pathsep + os.environ.get('PATH', '')) for command in item['commands']}
webkit = subprocess.run(['ldconfig', '-p'], capture_output=True, text=True, check=False)
optional['rich_ask'] = {'available': 'libwebkitgtk-6.0.so.4' in webkit.stdout, 'setup': 'Install the complete WebKitGTK 6 package for this distribution, including subprocess helpers and resources. Restart Koett. Plain Ask remains available without it; sandbox stays enabled.'}
print(json.dumps({'scope': 'current-PC only; Python 3 and host GTK dependencies required', 'required': checks, 'optional': optional}, indent=2))
raise SystemExit(0 if all(item['available'] for item in checks.values()) else 1)
