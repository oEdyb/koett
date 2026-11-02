"""Score dictionary approaches against the shared corpus.

Usage: python3 score.py transcripts.json [baseline.json]
transcripts.json maps "<clip id>-<voice>" to the final text an approach produced.
Metrics:
  term_recall    expected dictionary terms written exactly (case-sensitive, word bounded)
  false_terms    dictionary terms that appear in negative clips (none are spoken there)
  wer_positive / wer_negative   word error rate against the reference sentence
  broken_negatives   negative clips whose WER got worse than the baseline (if given)
  collateral_words   reference words outside the dictionary terms that the baseline
                     wrote and this output lost, over the positive clips (if given).
                     Catches biasing that rewrites neighbours ("invoices" -> "receipts").
"""
import json, re, sys
from collections import Counter
from pathlib import Path

CORPUS = json.loads((Path(__file__).parent / "corpus.json").read_text())

def has_term(text: str, term: str) -> bool:
    return re.search(r"(?<![\w-])" + re.escape(term) + r"(?![\w-])", text) is not None

def words(text: str) -> list[str]:
    return re.findall(r"[\w'-]+", text.lower())

def wer(ref: str, hyp: str) -> float:
    r, h = words(ref), words(hyp)
    d = list(range(len(h) + 1))
    for i in range(1, len(r) + 1):
        prev, d[0] = d[0], i
        for j in range(1, len(h) + 1):
            prev, d[j] = d[j], min(d[j] + 1, d[j - 1] + 1, prev + (r[i - 1] != h[j - 1]))
    return d[len(h)] / max(1, len(r))

def lost_words(ref: str, terms: list[str], base: str, out: str) -> int:
    """Non-term reference words present in the baseline but missing from the output."""
    term_words = {w for t in terms for w in words(t)}
    wanted = Counter(w for w in words(ref) if w not in term_words)
    had, has = Counter(words(base)), Counter(words(out))
    return sum(max(0, min(n, had[w]) - min(n, has[w])) for w, n in wanted.items())

def score(out: dict, base: dict | None = None) -> dict:
    hits = total = false_terms = broken = collateral = 0
    wp, wn, missing, false_list = [], [], [], []
    for voice in CORPUS["voices"]:
        for clip in CORPUS["positive"]:
            key = f"{clip['id']}-{voice}"
            text = out.get(key, "")
            for term in clip["terms"]:
                total += 1
                if has_term(text, term):
                    hits += 1
                else:
                    missing.append((key, term, text))
            wp.append(wer(clip["text"], text))
            if base is not None:
                collateral += lost_words(clip["text"], clip["terms"], base.get(key, ""), text)
        for clip in CORPUS["negative"]:
            key = f"{clip['id']}-{voice}"
            text = out.get(key, "")
            found = [t for t in CORPUS["dictionary"] if has_term(text, t)]
            false_terms += len(found)
            false_list += [(key, t, text) for t in found]
            w = wer(clip["text"], text)
            wn.append(w)
            if base is not None and w > wer(clip["text"], base.get(key, "")) + 1e-9:
                broken += 1
    result = {
        "term_recall": round(hits / total, 4), "term_hits": hits, "term_total": total,
        "false_terms": false_terms,
        "wer_positive": round(sum(wp) / len(wp), 4), "wer_negative": round(sum(wn) / len(wn), 4),
        "missing": missing, "false": false_list,
    }
    if base is not None:
        result["broken_negatives"] = broken
        result["collateral_words"] = collateral
    return result

if __name__ == "__main__":
    out = json.loads(Path(sys.argv[1]).read_text())
    base = json.loads(Path(sys.argv[2]).read_text()) if len(sys.argv) > 2 else None
    r = score(out, base)
    verbose = "-v" in sys.argv
    print(json.dumps({k: v for k, v in r.items() if verbose or k not in ("missing", "false")}, indent=1))
