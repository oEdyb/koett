#!/usr/bin/env bash
# Usage: transcribe.sh MODEL_DIR CLIP_DIR OUT.json [koett-engine binary]
set -euo pipefail
model=$1 clips=$2 out=$3
bin=${4:-$(dirname "$0")/../../CrossPlatform/target/release/koett-engine}
"$bin" "$model" "$clips"/*.wav 2>"$out.log" | python3 -c '
import json, sys, os
print(json.dumps({os.path.basename(p)[:-4]: t for p, t in (l.rstrip("\n").split("\t", 1) for l in sys.stdin)}, indent=1, ensure_ascii=False))' >"$out"
