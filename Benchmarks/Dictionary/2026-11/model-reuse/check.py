"""Recompute final model-reuse benchmark assertions from public synthetic audio."""
import json
from pathlib import Path
import statistics
import sys
root = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(root / 'Benchmarks/Dictionary'))
import score
base = Path(__file__).resolve().parent
records = json.loads((base / 'context.json').read_text())
assert len(records) == 210
assert json.loads((base / 'context.meta.json').read_text())['process_nice'] == 10
raw = {k: v['raw']['text'] for k, v in records.items()}
results = {m: score.score({k: v[m]['text'] for k, v in records.items()}, raw)
           for m in ['raw', 'cheap', 'dual4', 'context']}
assert [results[m]['term_hits'] for m in results] == [49, 61, 115, 115]
assert [(x[0], x[1]) for x in results['context']['missing']] == [(x[0], x[1]) for x in results['dual4']['missing']]
for result in results.values():
    assert result['false_terms'] == result['broken_negatives'] == result['collateral_words'] == 0
for row in records.values():
    assert row['context']['plain'] == row['empty_context']['plain'] == row['raw']['text']
print('210 clips: identical acoustic term recovery; both context base paths equal greedy')
for method in ['cheap', 'dual4', 'context']:
    print(method, sum(row[method]['ms'] for row in records.values()))
if (base / 'production.json').exists():
    production = json.loads((base / 'production.json').read_text())
    assert set(production) == {'5', '30', '120', '600'}
    for seconds, pairs in production.items():
        assert len(pairs) == 4
        values = {m: [p[m]['elapsed_ms'] for p in pairs[1:]] for m in ['empty', 'seed', 'legacy']}
        for pair in pairs:
            assert pair['empty']['actual_dictionary_mode'] == pair['seed']['actual_dictionary_mode'] == 'shared_encoder'
            assert pair['legacy']['actual_dictionary_mode'] == 'experimental_two_stream'
            assert all(pair[m]['process_nice'] == 10 for m in values)
        med = {m: statistics.median(v) for m, v in values.items()}
        print(seconds, {m: (med[m], max(v)) for m, v in values.items()}, 'added %', 100 * (med['seed']/med['empty']-1))
