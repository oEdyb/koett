"""Apply the reviewed NeMo encoder-sharing extension to exactly the pinned source."""

import hashlib
from pathlib import Path
import re

ORIGINAL_SHA256 = '561b99f9ad872be403c0839ee05eee59d87808fd5623d2e0ef0e6e61e31d0109'
PATCHED_SHA256 = '673caa2c3c3053158cc5c2fec576b17607ea0a053d648f34422591e0c689d920'


def apply_shared_context(contents):
    digest = hashlib.sha256(contents.encode()).hexdigest()
    if digest == PATCHED_SHA256:
        return contents
    if digest != ORIGINAL_SHA256:
        raise ValueError('unrecognized NeMo source; review encoder-sharing patch before upgrading')
    source = contents.splitlines(keepends=True)
    output, cursor = [], 0
    patch = Path(__file__).with_name('shared_context.patch').read_text().splitlines(keepends=True)
    for line in patch[2:]:
        if line.startswith('@@'):
            start = int(re.match(r'@@ -(\d+)', line)[1]) - 1
            output.extend(source[cursor:start])
            cursor = start
        elif line.startswith(('+', '-', ' ')):
            if line[0] != '+':
                if source[cursor] != line[1:]:
                    raise ValueError('NeMo patch context does not match')
                cursor += 1
            if line[0] != '-':
                output.append(line[1:])
    output.extend(source[cursor:])
    result = ''.join(output)
    if hashlib.sha256(result.encode()).hexdigest() != PATCHED_SHA256:
        raise ValueError('NeMo patch output checksum mismatch')
    return result
