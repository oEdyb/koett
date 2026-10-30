#!/usr/bin/env python3

import argparse
from pathlib import Path

from benchmark_record import alignment, normalize_words


parser = argparse.ArgumentParser(description="Calculate normalized word error rate.")
parser.add_argument("reference", type=Path)
parser.add_argument("hypothesis", type=Path)
arguments = parser.parse_args()

reference = normalize_words(arguments.reference.read_text(encoding="utf-8"))
hypothesis = normalize_words(arguments.hypothesis.read_text(encoding="utf-8"))
if not reference:
    parser.error("the normalized reference has no words; WER is undefined")
edits = alignment(reference, hypothesis)["errors"]
wer = edits / len(reference)
print(
    f"reference_words={len(reference)} hypothesis_words={len(hypothesis)} "
    f"edits={edits} WER={wer:.6f} ({wer * 100:.3f}%)"
)
