#!/usr/bin/env python3
import json
import os
from pathlib import Path
import sys
import time

mode = Path(sys.argv[0]).name
assert os.environ.get("HF_HUB_OFFLINE") == "1"
job = Path(sys.argv[1]).parent
(job.parent / "helper.pgid").write_text(str(os.getpgrp()))
if mode == "no-ready":
    sys.exit(2)
print(json.dumps({
    "ready": True, "device": "cuda",
    "models": {"kb-whisper-large": "/swedish", "whisper-large-v3-turbo": "/turbo"},
}), flush=True)

seen = 0
previous_id = None
for line in sys.stdin:
    batch = json.loads(line)
    assert batch["op"] == "decode"
    for window in batch["windows"]:
        identifier = window["id"]
        assert Path(window["pcm"]).stat().st_size > 0
        if seen == 2:
            if mode == "crash":
                os._exit(42)
            if mode == "hang":
                time.sleep(3600)
            if mode == "error":
                print(json.dumps({"id": identifier, "error": "simulated CUDA failure"}), flush=True)
                sys.exit(0)
        unsupported_language = mode == "unknown-language" and seen == 2
        reply = json.dumps({
            "id": previous_id if mode == "duplicate" and seen == 2 else identifier,
            "lang": "sl" if unsupported_language else "sv", "lang_p": 0.9,
            "backend": "whisper-large-v3-turbo" if unsupported_language or (mode == "wrong-backend" and seen == 2) else "kb-whisper-large",
            "words": [["Hej", -1.0 if mode == "broken-timestamps" and seen == 2 else 0.4, 0.6]],
        })
        if mode == "split" and seen == 0:
            print(reply[:10], end="", flush=True)
            time.sleep(0.075)
            reply = reply[10:]
        print(reply, flush=True)
        previous_id = identifier
        seen += 1
