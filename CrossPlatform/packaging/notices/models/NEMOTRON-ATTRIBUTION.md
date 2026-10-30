# Nemotron Speech Streaming 0.6B attribution

Original model: NVIDIA, nvidia/nemotron-speech-streaming-en-0.6b.
Original model reference: https://huggingface.co/nvidia/nemotron-speech-streaming-en-0.6b
License: NVIDIA Open Model License, retained in NVIDIA-Open-Model-License.html.
See nemotron-model-card.md for the pinned original NVIDIA model card.

Downloaded conversion: sherpa-onnx-nemotron-speech-streaming-en-0.6b-560ms-int8-2026-04-25,
from k2-fsa/sherpa-onnx's asr-models release, SHA256
78e2b79fcf7271553a74402a76b771b09ea40117a39566a79f52235b23db6358.

Exporter copyright: 2026 Xiaomi Corp.; author: Fangjun Kuang, as written in
nemotron-provenance/export_onnx.py. The exporter and release workflow are from
sherpa-onnx source commit 3dc7c569f31ca2cd4a20ed6f7db780327e6714c5.
They show ONNX export of the transducer branch, 560ms chunk configuration,
and dynamic QUInt8 quantization of encoder, decoder and joiner. The exporter
writes model_author=NeMo and the NVIDIA original model URL into ONNX metadata.
These are facts about the published exporter; Koett did not independently
recreate the converted model or inspect its weight files in this review.

The converted model's published README credits the NVIDIA model and chunk
size; a pinned copy is in nemotron-provenance/converted-model-README.md.
That README does not add a model license or identify an immutable original
NVIDIA checkpoint. The exporter fetches the NVIDIA model by a mutable name.
Its source-code copyright does not establish separate license terms for the
converted model weights. These notices do not imply endorsement or approval
by NVIDIA, Xiaomi, Fangjun Kuang or k2-fsa.

## Unresolved upstream question

For the exact release artifact SHA256 above, which immutable NVIDIA checkpoint
was converted, and what conversion-specific copyright/attribution or license
notices, if any, must accompany redistribution of the converted weights and
included test audio? Please confirm the original NVIDIA model license applies
to that artifact and identify any additional notices required by its converter.

No model bytes are bundled by Koett. These factual credits preserve the
canonical manifest's explicit conversion-attribution clarification gap;
they do not claim upstream clearance or resolve that gap. An application-only
archive may carry these notices, but describing every supported model download
as cleared for redistribution remains outside this review's established scope.
