"""Plot the measured public dictionary corpus; no private transcript input."""
from pathlib import Path
import json
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

root = Path(__file__).resolve().parents[3]
receipt = root / 'Benchmarks/Dictionary/2026-11/model-reuse/context.json'
records = json.loads(receipt.read_text())
methods = ['cheap', 'dual4', 'context']
seconds = [sum(row[method]['ms'] for row in records.values()) / 1000 for method in methods]
fig, ax = plt.subplots(figsize=(10, 4.6))
fig.subplots_adjust(left=.27, right=.97, top=.84, bottom=.24)
bars = ax.barh(['Spelling only', 'Previous acoustic dictionary', 'Shared encoder'], seconds,
               color=['#a9aaa6', '#d7aa91', '#8aa58e'], height=.55)
ax.invert_yaxis()
ax.set_xlim(0, 44)
for bar, value, hits in zip(bars, seconds, [61, 115, 115]):
    ax.text(value - .5, bar.get_y() + bar.get_height()/2,
            f'{value:.2f} s · {hits}/162 terms', va='center', ha='right', fontsize=11)
ax.set_xlabel('Total inference wall time on CPU (seconds)')
ax.spines[['top', 'right', 'left']].set_visible(False)
ax.tick_params(axis='y', length=0, labelsize=11)
ax.grid(axis='x', color='#e5e5e2', linewidth=.6)
ax.set_axisbelow(True)
fig.suptitle('Same acoustic term recall, 44.0% less processing time', x=.02, y=.97, ha='left', fontsize=15)
fig.text(.02, .03, 'Ryzen 5 9600X · 2 CPU threads · pinned Parakeet v2 · 30 terms · one run per method per clip\nFile ASR, not stop-to-paste. Synthetic corpus used during development; no human-speech accuracy claim.', fontsize=9)
fig.savefig(Path(__file__).with_name('dictionary-cpu.png'), dpi=160, bbox_inches='tight', facecolor='white')
