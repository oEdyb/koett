"""Recompute acceptance from public synthetic-corpus receipts; no inference."""
import json
import math
from pathlib import Path
import statistics
import sys

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT.parent))
from score import CORPUS, score  # noqa: E402


def main():
    records = json.loads((ROOT / 'final-context.json').read_text())
    expected = {f"{clip['id']}-{voice}" for group in ['positive', 'negative']
                for clip in CORPUS[group] for voice in CORPUS['voices']}
    assert set(records) == expected
    header = json.loads((ROOT / 'final-context.meta.json').read_text())['header_sha256']
    assert json.loads((ROOT / 'native-build-receipt.json').read_text())['shared_context_sha256'] == header
    raw = {key: row['raw']['text'] for key, row in records.items()}
    scored = {}
    for method in ['raw', 'cheap', 'dual4', 'context']:
        output = {key: row[method]['text'] for key, row in records.items()}
        scored[method] = score(output, raw)
        assert all(math.isfinite(row[method]['ms']) and row[method]['ms'] > 0
                   for row in records.values())
    assert all(row['context']['plain'] == row['raw']['text'] for row in records.values())
    assert scored['dual4']['term_hits'] == scored['context']['term_hits'] == 115
    # The scorer's public summary omits missing details; compare term decisions directly.
    from score import has_term
    for clip in CORPUS['positive']:
        for voice in CORPUS['voices']:
            row = records[f"{clip['id']}-{voice}"]
            for term in clip['terms']:
                assert has_term(row['context']['text'], term) == has_term(row['dual4']['text'], term)
    for field in ['false_terms', 'broken_negatives', 'collateral_words']:
        assert scored['context'][field] == 0
    totals = {m: sum(row[m]['ms'] for row in records.values()) for m in scored}
    overheads = sorted(100 * (row['context']['ms'] / row['cheap']['ms'] - 1)
                       for row in records.values())
    print('Corpus: 210 complete clips; exact greedy base; same 115/162 terms; no collateral loss')
    print(f"Aggregate added time: {100 * (totals['context'] / totals['cheap'] - 1):.3f}%")
    print(f'Per-clip maximum added time: {max(overheads):.3f}% (one run per method)')
    lengths = json.loads((ROOT / 'final-production-lengths.json').read_text())
    assert set(lengths) == {'5', '30', '120', '600'}
    for seconds, pairs in lengths.items():
        assert len(pairs) == 4  # first pair is warm-up
        assert all(p['seed']['mode'] == 'shared_encoder' and p['seed']['header_sha256'] == header for p in pairs)
        median = lambda method: statistics.median(p[method]['elapsed_ms'] for p in pairs[1:])
        overhead = 100 * (median('seed') / median('empty') - 1)
        assert overhead < 10, (seconds, overhead)
        print(f'{seconds}s: {median("empty"):.3f} -> {median("seed"):.3f} ms, +{overhead:.3f}%')

    loaded = json.loads((ROOT / 'production-three-modes.json').read_text())
    for seconds, rounds in loaded.items():
        med = lambda mode: statistics.median(r[mode]['elapsed_ms'] for r in rounds[1:])
        overhead = 100 * (med('seed') / med('empty') - 1)
        print(f'Contended {seconds}s: +{overhead:.2f}% added; {100*(1-med("seed")/med("legacy")):.2f}% less time vs old')
        if overhead > 10:
            print('WARNING: exceeds nominal 10% target under contention')
    scaling = json.loads((ROOT / 'scaling-shared.json').read_text())
    large = 100 * (scaling['2000']['warm_median_ms'] / scaling['0']['warm_median_ms'] - 1)
    print(f'Large 2,000-term vocabulary: +{large:.2f}% (outside the nominal target)')


if __name__ == '__main__':
    main()
