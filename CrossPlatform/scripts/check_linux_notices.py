#!/usr/bin/env python3
"""Validate canonical distributable notice bytes against reviewed provenance."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REQUIRED = [
    'rust/sherpa-onnx-1.13.5/KOETT-MODIFICATIONS.txt',
    'models/CC-BY-4.0.html', 'models/NVIDIA-Open-Model-License.html',
    'models/NEMOTRON-ATTRIBUTION.md', 'models/nemotron-model-card.md',
    'native/silero-vad/LICENSE', 'native/onnxruntime/ThirdPartyNotices.txt',
    'native/onnxruntime/provenance.json', 'native/onnxruntime/SOURCE-ACCESS.md',
    'native/onnxruntime/dependencies/eigen/COPYING.MPL2',
    'native/onnxruntime/build-provenance/cmake/patches/eigen/s390x-build.patch',
    'native/onnxruntime/build-provenance/cmake/patches/eigen/s390x-build-werror.patch',
    'native/openfst/COPYING', 'native/gtk4/COPYING', 'components.json', 'models.json',
    'native/kissfft/LICENSES/BSD-3-Clause',
    'renderer/THIRD-PARTY-NOTICES.md', 'renderer/marked-LICENSE.txt',
    'renderer/katex-LICENSE.txt', 'renderer/highlight-LICENSE.txt',
    'renderer/dompurify-LICENSE.txt', 'renderer/dompurify-LICENSE-MPL.txt',
    'renderer/katex-font-OFL-1.1.txt', 'renderer/katex-font-copyrights.txt',
    'renderer/katex-font-metadata.json', 'release-review-sources.json',
]


def validate(root=ROOT):
    folder = root / 'packaging/notices'
    index = json.loads((folder / 'files.json').read_text())
    expected = {item['path']: item for item in index['files']}
    actual = {str(p.relative_to(folder)): p for p in folder.rglob('*') if p.is_file() and p != folder / 'files.json'}
    if expected.keys() != actual.keys():
        raise ValueError('notice file set differs from provenance index')
    for name, path in actual.items():
        data = path.read_bytes()
        if not data or hashlib.sha256(data).hexdigest() != expected[name]['sha256']:
            raise ValueError(f'notice content mismatch: {name}')
    for required in REQUIRED:
        if required not in actual:
            raise ValueError(f'required notice missing: {required}')
    components = json.loads((folder / 'components.json').read_text())['components']
    sources = json.loads((folder / 'release-review-sources.json').read_text())['sources']
    for item in [file for component in components for file in component['files']] + sources:
        name = item['path'].removeprefix('packaging/notices/')
        if name not in expected or item['sha256'] != expected[name]['sha256']:
            raise ValueError(f'notice provenance mismatch: {name}')
    fonts = json.loads((folder / 'renderer/katex-font-metadata.json').read_text())
    font_root = root.parent / 'Sources/Koett/Resources/AssistantRenderer/fonts'
    recorded = {Path(item['path']).name for item in fonts}
    if recorded != {path.name for path in font_root.glob('*.woff2')}:
        raise ValueError('embedded font set differs from reviewed notice metadata')
    for item in fonts:
        path = font_root / Path(item['path']).name
        if hashlib.sha256(path.read_bytes()).hexdigest() != item['sha256']:
            raise ValueError(f'embedded font differs from reviewed notice metadata: {path.name}')
    return len(actual)


if __name__ == '__main__':
    print(f'Validated {validate()} notice files')
