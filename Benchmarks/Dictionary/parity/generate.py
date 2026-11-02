"""Regenerate Tests/KoettTests/Dictionary/Fixtures/parity.json from the Rust dictionary code.

Usage: python3 generate.py BENCH_OUT_DIR
BENCH_OUT_DIR holds mac-baseline.json and mac-boosted-*.json from DictionaryBenchmarkTests.
"""
import glob, json, os, subprocess, sys
from pathlib import Path

here = Path(__file__).parent
root = here.parents[2]
bench = Path(sys.argv[1])
subprocess.run(["cargo", "build", "--release", "--quiet"], cwd=here, check=True)
corpus = json.loads((here.parent / "corpus.json").read_text())
forms = {"Supabase": ["super base", "Superbase"], "Groq": ["grok"], "Koett": ["cot", "quett"],
         "Olle": ["all"], "Fortnox": ["four knocks"], "Göteborg": ["gataborg"]}
dictionary = [{"term": t, "sounds_like": forms.get(t, [])} for t in corpus["dictionary"]]
dictionary += [{"term": t, "sounds_like": []} for t in ["C++", "C#", ".NET", "Node.js", "Olle Dyberg", "Claude Code"]]
base = json.loads((bench / "mac-baseline.json").read_text())
outs = [json.loads(Path(p).read_text()) for p in sorted(glob.glob(str(bench / "mac-boosted*.json")))]
refs = {f"{c['id']}-{v}": c["text"] for k in ("positive", "negative") for c in corpus[k] for v in corpus["voices"]}
keys = sorted(base)
spell = sorted(set([c["text"] for k in ("positive", "negative") for c in corpus[k]]
                   + [base[k] for k in keys] + [base[k].lower() for k in keys] + [o[k] for o in outs for k in keys]))
spell += ["c++ code, C++ and c# on .net", "Use node.js and göteborg.dev", "all Dyberg and all",
          "Super. Base", "claude code and the cloud"]
adopt = sorted(set([(base[k], o[k]) for o in outs for k in keys if base[k] != o[k]]
                   + [(base[k], refs[k]) for k in keys] + [(refs[k], base[k]) for k in keys]))
learn = sorted(set([(base[k], refs[k]) for k in keys] + [(refs[k], base[k]) for k in keys]))
data = {"dictionary": dictionary, "spell": spell, "adopt": adopt, "learn": learn}
result = subprocess.run([str(here / "target/release/koett-parity")], input=json.dumps(data).encode(),
                        capture_output=True, check=True)
commit = subprocess.check_output(["git", "rev-parse", "--short", "HEAD"], cwd=root).decode().strip()
out = root / "Tests/KoettTests/Dictionary/Fixtures/parity.json"
out.write_text(json.dumps({"source": f"CrossPlatform/src at {commit}", "input": data,
                           "expected": json.loads(result.stdout)}, ensure_ascii=False, indent=0))
print(f"{len(spell)} spell, {len(adopt)} adopt, {len(learn)} learn cases -> {out}")
