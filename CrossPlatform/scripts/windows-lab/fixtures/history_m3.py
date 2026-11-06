"""Create the small native History fixture; never point this at a user's DB."""
import sqlite3
import sys
from pathlib import Path

path = Path(sys.argv[1])
if path.exists():
    raise SystemExit('Refusing to overwrite an existing file')
repo = Path(__file__).resolve().parents[4]
with sqlite3.connect(path) as database:
    database.executescript((repo / 'Sources/Koett/Resources/History/schema.sql').read_text())
    database.executemany('INSERT INTO transcripts VALUES (?,?,?,?,?)', [
        (f'm3-{i:03}', 1700000000000+i, 'fixture', f'M3 final {i:03} ÅÄÖ',
         f'M3 raw {i:03} åäö' if i != 109 else None)
        for i in range(110)
    ])
