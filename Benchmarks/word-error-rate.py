#!/usr/bin/env python3

import argparse
import unicodedata
from pathlib import Path


def words(text: str) -> list[str]:
    text = text.upper().replace("’", "'")
    normalized = []
    for character in text:
        if character == "'" or character.isalnum() or character.isspace():
            normalized.append(character)
        elif unicodedata.category(character).startswith("P"):
            normalized.append(" ")
        else:
            normalized.append(" ")
    return "".join(normalized).split()


def edit_distance(reference: list[str], hypothesis: list[str]) -> int:
    row = list(range(len(hypothesis) + 1))
    for reference_index, reference_word in enumerate(reference, 1):
        next_row = [reference_index]
        for hypothesis_index, hypothesis_word in enumerate(hypothesis, 1):
            next_row.append(
                min(
                    next_row[-1] + 1,
                    row[hypothesis_index] + 1,
                    row[hypothesis_index - 1] + (reference_word != hypothesis_word),
                )
            )
        row = next_row
    return row[-1]


parser = argparse.ArgumentParser(description="Calculate normalized word error rate.")
parser.add_argument("reference", type=Path)
parser.add_argument("hypothesis", type=Path)
arguments = parser.parse_args()

reference = words(arguments.reference.read_text(encoding="utf-8"))
hypothesis = words(arguments.hypothesis.read_text(encoding="utf-8"))
if not reference:
    parser.error("the normalized reference has no words; WER is undefined")
edits = edit_distance(reference, hypothesis)
wer = edits / len(reference)
print(
    f"reference_words={len(reference)} hypothesis_words={len(hypothesis)} "
    f"edits={edits} WER={wer:.6f} ({wer * 100:.3f}%)"
)
