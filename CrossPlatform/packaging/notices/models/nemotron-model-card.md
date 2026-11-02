---
license: other
license_name: nvidia-open-model-license
license_link: >-
  https://www.nvidia.com/en-us/agreements/enterprise-software/nvidia-open-model-license/
library_name: nemo
datasets:
- nvidia/Granary
- YTC
- Yodas2
- LibriLight
- librispeech_asr
- fisher_corpus
- Switchboard-1
- WSJ-0
- WSJ-1
- National-Singapore-Corpus-Part-1
- National-Singapore-Corpus-Part-6
- vctk
- voxpopuli
- europarl
- multilingual_librispeech
- fleurs
- mozilla-foundation/common_voice_8_0
- MLCommons/peoples_speech
- google/speech_commands
thumbnail: null
tags:
- transformers
- speech-recognition
- cache-aware ASR
- automatic-speech-recognition
- streaming-asr
- speech
- audio
- FastConformer
- RNNT
- Parakeet
- ASR
- pytorch
- NeMo
- hf-asr-leaderboard
widget:
- example_title: Librispeech sample 1
  src: https://cdn-media.huggingface.co/speech_samples/sample1.flac
- example_title: Librispeech sample 2
  src: https://cdn-media.huggingface.co/speech_samples/sample2.flac
model-index:
- name: nemotron-speech-streaming-en-0.6b
  results:
  - task:
      name: Automatic Speech Recognition
      type: automatic-speech-recognition
    dataset:
      name: AMI
      type: ami
      config: ihm
      split: test
    metrics:
    - name: WER (1.12s frame size)
      type: wer
      value: 11.73
  - task:
      name: Automatic Speech Recognition
      type: automatic-speech-recognition
    dataset:
      name: Earnings22
      type: earnings22
      split: test
    metrics:
    - name: WER (1.12s frame size)
      type: wer
      value: 12.52
  - task:
      name: Automatic Speech Recognition
      type: automatic-speech-recognition
    dataset:
      name: Gigaspeech
      type: gigaspeech
      split: test
    metrics:
    - name: WER (1.12s frame size)
      type: wer
      value: 9.66
  - task:
      name: Automatic Speech Recognition
      type: automatic-speech-recognition
    dataset:
      name: LibriSpeech test-clean
      type: librispeech_asr
      config: clean
      split: test
    metrics:
    - name: WER (1.12s frame size)
      type: wer
      value: 2.32
  - task:
      name: Automatic Speech Recognition
      type: automatic-speech-recognition
    dataset:
      name: LibriSpeech test-other
      type: librispeech_asr
      config: other
      split: test
    metrics:
    - name: WER (1.12s frame size)
      type: wer
      value: 4.84
  - task:
      name: Automatic Speech Recognition
      type: automatic-speech-recognition
    dataset:
      name: SPGI Speech
      type: spgispeech
      split: test
    metrics:
    - name: WER (1.12s frame size)
      type: wer
      value: 2.97
  - task:
      name: Automatic Speech Recognition
      type: automatic-speech-recognition
    dataset:
      name: TEDLIUM
      type: tedlium
      split: test
    metrics:
    - name: WER (1.12s frame size)
      type: wer
      value: 3.5
  - task:
      name: Automatic Speech Recognition
      type: automatic-speech-recognition
    dataset:
      name: VoxPopuli
      type: voxpopuli
      config: en
      split: test
    metrics:
    - name: WER (1.12s frame size)
      type: wer
      value: 7.97
metrics:
- wer
pipeline_tag: automatic-speech-recognition
---


# Nemotron ASR Streaming

<style>
h1, h2, h3, h4, h5, h6 {
  color: #76b900; /* NVIDIA green */
  font-weight: 700;
}

hr {
  border: none;
  border-top: 1px solid #e5e7eb;
  margin: 2rem 0;
}

/* Improve list spacing */
ul, ol {
  margin-top: 0.5rem;
  margin-bottom: 0.5rem;
}

/* Badge alignment consistency */
img {
  display: inline;
  vertical-align: middle;
}


</style>

[![Model architecture](https://img.shields.io/badge/Model_Arch-FastConformer--CacheAware--RNNT-lightgrey#model-badge)](#model-architecture)
| [![Model size](https://img.shields.io/badge/Params-600M-lightgrey#model-badge)](#model-architecture)
| [![Language](https://img.shields.io/badge/Language-English-lightgrey#model-badge)](#datasets)

> [!Note]
> June 4, 2026: **New multilingual model released:** [NVIDIA Nemotron 3.5 ASR Streaming 0.6B](https://huggingface.co/nvidia/nemotron-3.5-asr-streaming-0.6b) extends this English streaming ASR model to **40 language-locales** in a single 600M-parameter model. It supports language-ID prompt conditioning, optional automatic language detection, punctuation and capitalization, and configurable low-latency streaming chunk sizes.
>
> March 12, 2026: nemotron-asr-streaming was released with updated checkpoint (trained on larger corpora). For the older checkpoint released in January 2026, please refer to the [nemotron-speech-streaming-jan2026](https://huggingface.co/nvidia/nemotron-speech-streaming-en-0.6b/tree/nemotron-speech-streaming-jan2026) branch.


Nemotron-ASR-Streaming is an English, streaming Automatic Speech Recognition (ASR) engineered to deliver high-quality English transcription across both low-latency streaming and high-throughput batch workloads. Developed by NVIDIA, this 600M parameter model transcribes speech into text with native support for punctuation and capitalization and offers runtime flexibility with configurable chunk sizes, including 80ms, 160ms, 560ms, and 1120ms.

By leveraging the state-of-the-art Cache-Aware FastConformer-RNNT architecture, the model eliminates redundant overlapping computations common in traditional "buffered" streaming. This allows it to process only new audio chunks while reusing cached encoder context, significantly improving computational efficiency and minimizing end-to-end delay without sacrificing accuracy.

It transcribes speech into the English alphabet, spaces, and apostrophes, with full support for punctuation and capitalization. Trained on the ASRSet, a massive dataset of approximately 250,000 hours of US English (en-US) speech, it is engineered to perform across diverse and challenging acoustic conditions.


Why Choose nvidia/nemotron-asr-streaming?

* **Native Streaming Architecture:** Cache-aware design enables efficient processing of continuous audio streams, designed and optimized for low-latency voice agent applications interaction.
* **Improved Operational Efficiency:** Delivers superior throughput compared to traditional buffered streaming approaches. This allows for a higher number of parallel streams within the same GPU memory constraints, directly reducing operational costs for production environments.
* **Dynamic Runtime Flexibility:** Enables you to choose the optimal operating point on the latency-accuracy Pareto curve at inference time. No re-training is required to adjust for different use-case requirements.
* **Punctuation & Capitalization:** Built-in support for punctuation and capitalization in output text

<figure align="center">
  <img src="figures/results_wer_and_scaling.png" width="1250" />
  <figcaption>
    Nemotron-asr-streaming, outperforming Jan26 version, allows users to choose the optimal operating point on the latency-accuracy pareto curve at inference time, without requiring any re-training. Further, the cache-aware streaming mechanism scales much better than buffered streaming approaches, consistently outperforming production models like <a href="https://build.nvidia.com/nvidia/parakeet-ctc-1_1b-asr"> parakeet-ctc-1_1b-asr </a> across chunk sizes.
  </figcaption>
</figure>
  
This model consists of a cache-aware streaming 🦜 Parakeet (FastConformer) encoder with an RNN-T decoder. It is designed for real-time speech-to-text applications where low latency is critical, such as voice assistants, live captioning, and conversational AI systems. Unlike traditional "buffered" streaming, the cache-aware architecture enables continuous transcription by processing only new audio chunks while reusing cached encoder context. This significantly improves computational efficiency and minimizes end-to-end delay without sacrificing accuracy.

🗣️ **Experience `Nemotron-Speech-Streaming-En-0.6b` in action** here: [https://huggingface.co/spaces/nvidia/nemotron-speech-streaming-en-0.6b](https://huggingface.co/spaces/nvidia/nemotron-speech-streaming-en-0.6b)

This model is ready for commercial/non-commercial use.

Read more about the model in the [dev blog](https://huggingface.co/blog/nvidia/nemotron-speech-asr-scaling-voice-agents) and check out the [paper](https://arxiv.org/abs/2312.17279).


<div align="center">
    <img src="figures/inference_pipeline.png" width="750" />
</div>

## License/Terms of Use:
Governing Terms: Use of the model is governed by the [NVIDIA Open Model License Agreement](https://www.nvidia.com/en-us/agreements/enterprise-software/nvidia-open-model-license/).

## Deployment Geography:
Global <br>

## Use Case: <br>
This model is for transcription of English audio. <br>

## Release Date:  <br>
- Build.Nvidia.com [03/13/2026] via https://build.nvidia.com/nvidia/nemotron-asr-streaming/ <br> 
- Hugging Face [03/13/2026] via https://huggingface.co/nvidia/nemotron-speech-streaming-en-0.6b <br> 
(older checkpoint [01/05/2026] via https://huggingface.co/nvidia/nemotron-speech-streaming-en-0.6b/tree/nemotron-speech-streaming-jan2026) <br>
- NGC [03/13/2026] via https://catalog.ngc.nvidia.com/orgs/nvidia/collections/nemotron-asr-streaming <br>

## Explore more from NVIDIA:
For documentation, deployment guides, enterprise-ready APIs, and the latest open models—including Nemotron and other cutting-edge speech, translation, and generative AI—visit the NVIDIA Developer Portal at [developer.nvidia.com](https://developer.nvidia.com/).
Join the community to access tools, support, and resources to accelerate your development with NVIDIA's NeMo, Riva, NIM, and foundation models.<br>

What is [Nemotron](https://www.nvidia.com/en-us/ai-data-science/foundation-models/nemotron/)?<br>
NVIDIA Developer [Nemotron](https://developer.nvidia.com/nemotron)<br>
[NVIDIA Riva Speech](https://developer.nvidia.com/riva?sortBy=developer_learning_library%2Fsort%2Ffeatured_in.riva%3Adesc%2Ctitle%3Aasc#demos)<br>
[NeMo Documentation](https://docs.nvidia.com/nemo-framework/user-guide/latest/nemotoolkit/asr/models.html)<br>

Also, check out the following NVIDIA speech models that extend the capabilities of Nemotron-Speech-Streaming:
* Multitalker Parakeet Streaming - https://huggingface.co/nvidia/multitalker-parakeet-streaming-0.6b-v1
* Parakeet Realtime EOU - https://huggingface.co/nvidia/parakeet_realtime_eou_120m-v1

## Access Model Inference and Examples:
* [NVIDIA NIM](https://build.nvidia.com/nvidia/nemotron-asr-streaming)
* Deploy the Nemotron Speech ASR endpoint on [Modal](https://github.com/modal-projects/modal-nvidia-asr)
* Build local voice agent using [Daily’s framework](https://github.com/pipecat-ai/nemotron-january-2026)

## Model Architecture

**Architecture Type:** FastConformer-CacheAware-RNNT

The model is based on the Cache-Aware [1] FastConformer [2] architecture with 24 encoder layers and an RNNT (Recurrent Neural Network Transducer) decoder. The cache-aware streaming design enables efficient processing of audio in chunks while maintaining context from previous frames. Unlike buffered inference, this model maintains caches for all encoder self-attention and convolution layers. This enables reuse of hidden states at every streaming step, where cached activations eliminate redundant computations. As a result, there are no overlapping computations; each processed frame is strictly non-overlapping.

The caching schema of self-attention and convolution layers for consecutive chunks is as follows. For more details, please refer to [1].
<div align="center">
    <img src="figures/caching_schema.png" width="750" />
</div>

**Network Architecture:**
- Encoder: Cache-Aware FastConformer with 24 layers
- Decoder: RNNT (Recurrent Neural Network Transducer)
- Parameters: 600M

## How to Use this Model

There are several ways to use this model. Choose the one that fits your needs.

### Run locally with NeMo-Speech.cpp

[NeMo-Speech.cpp](https://github.com/NVIDIA/NeMo-Speech.cpp) provides a
lightweight native C++ runtime for local inference with
this model. After [installing the runtime](https://github.com/NVIDIA/NeMo-Speech.cpp#installation):

```bash
hf download nvidia/nemotron-speech-streaming-en-0.6b \
  nemotron-speech-streaming-en-0.6b.q8_0.gguf \
  --local-dir models

nemo-speech transcribe audio.wav \
  --model models/nemotron-speech-streaming-en-0.6b.q8_0.gguf
```

See the [NeMo-Speech.cpp documentation](https://github.com/NVIDIA/NeMo-Speech.cpp)
for more details.

### NVIDIA NeMo

To train, fine-tune or perform inference with this model, you will need to install [NVIDIA NeMo](https://github.com/NVIDIA/NeMo)[4]. We recommend you install it after you've installed Cython and latest PyTorch version.

```bash
apt-get update && apt-get install -y libsndfile1 ffmpeg
pip install Cython packaging
pip install git+https://github.com/NVIDIA/NeMo.git@main#egg=nemo_toolkit[asr]
```

#### Loading the Model

```python
import nemo.collections.asr as nemo_asr
asr_model = nemo_asr.models.ASRModel.from_pretrained(model_name="nvidia/nemotron-speech-streaming-en-0.6b")
```

#### Streaming Inference
You can use the cache-aware streaming inference script from NeMo - [NeMo/examples/asr/asr_cache_aware_streaming/speech_to_text_cache_aware_streaming_infer.py](https://github.com/NVIDIA-NeMo/NeMo/blob/main/examples/asr/asr_cache_aware_streaming/speech_to_text_cache_aware_streaming_infer.py)

``` bash
cd NeMo
python examples/asr/asr_cache_aware_streaming/speech_to_text_cache_aware_streaming_infer.py \
    model_path=<model_path> \
    dataset_manifest=<dataset_manifest> \ 
    batch_size=<batch_size> \
    att_context_size="[70,13]" \ #set the second value to the desired right context from {0,1,6,13}
    output_path=<output_folder> 
```


You can also run streaming inference through the pipeline method, which uses [NeMo/examples/asr/conf/asr_streaming_inference/cache_aware_rnnt.yaml](https://github.com/NVIDIA-NeMo/NeMo/blob/main/examples/asr/conf/asr_streaming_inference/cache_aware_rnnt.yaml) configuration file to build end‑to‑end workflows with punctuation and capitalization (PnC), inverse text normalization (ITN), and translation support.

```python
from nemo.collections.asr.inference.factory.pipeline_builder import PipelineBuilder
from omegaconf import OmegaConf

# Path to the cache aware config file downloaded from above link
cfg_path = 'cache_aware_rnnt.yaml'
cfg = OmegaConf.load(cfg_path)

# Pass the paths of all the audio files for inferencing
audios = ['/path/to/your/audio.wav']

# Create the pipeline object and run inference
pipeline = PipelineBuilder.build_pipeline(cfg)
output = pipeline.run(audios)

# Print the output
for entry in output:
  print(entry['text'])
```

_______

#### Setting up Streaming Configuration

Latency is defined by the `att_context_size` param, where  att_context_size = `{num_frames_left_context, num_frame_right_context}`, all measured in **80ms frames**:
* [70, 0]: Chunk size = 1 (1 × 80ms = 0.08s)
* [70, 1]: Chunk size = 2 (2 × 80ms = 0.16s)
* [70, 6]: Chunk size = 7 (7 × 80ms = 0.56s)
* [70, 13]: Chunk size = 14 (14 × 80ms = 1.12s)

Here, chunk size = current frame + right context; each chunk is processed in non-overlapping fashion.


### 🤗 Transformers usage

This checkpoint also runs with [🤗 Transformers](https://github.com/huggingface/transformers).

Nemotron ASR Streaming is available in 🤗 Transformers starting from v5.13.0.

```bash
pip install "transformers>=5.13.0"
```

<details>
  <summary>➡️ Pipeline</summary>

```python
from transformers import pipeline

pipe = pipeline("automatic-speech-recognition", model="nvidia/nemotron-speech-streaming-en-0.6b")
out = pipe("https://huggingface.co/datasets/hf-internal-testing/dummy-audio-samples/resolve/main/bcn_weather.mp3")
print(out)
```
</details>

<details>
  <summary>➡️ Offline transcription</summary>

```python
from transformers import AutoModelForRNNT, AutoProcessor
from transformers.audio_utils import load_audio

model_id = "nvidia/nemotron-speech-streaming-en-0.6b"
processor = AutoProcessor.from_pretrained(model_id)
model = AutoModelForRNNT.from_pretrained(model_id, device_map="auto")

audio = load_audio(
    "https://huggingface.co/datasets/hf-internal-testing/dummy-audio-samples/resolve/main/bcn_weather.mp3",
    sampling_rate=processor.feature_extractor.sampling_rate,
)

inputs = processor(audio, sampling_rate=processor.feature_extractor.sampling_rate)
inputs.to(model.device, dtype=model.dtype)
output = model.generate(**inputs, return_dict_in_generate=True)
print(processor.decode(output.sequences, skip_special_tokens=True))
```
</details>

<details>
  <summary>➡️ Streaming transcription</summary>

```python
from threading import Thread
from transformers import AutoModelForRNNT, AutoProcessor, TextIteratorStreamer
from transformers.audio_utils import load_audio

model_id = "nvidia/nemotron-speech-streaming-en-0.6b"
processor = AutoProcessor.from_pretrained(model_id)
model = AutoModelForRNNT.from_pretrained(model_id, device_map="auto")

processor.set_num_lookahead_tokens(6)
print(f"Streaming latency: {processor.streaming_latency_ms} ms")

sampling_rate = processor.feature_extractor.sampling_rate
audio = load_audio(
    "https://huggingface.co/datasets/hf-internal-testing/dummy-audio-samples/resolve/main/obama.mp3",
    sampling_rate=sampling_rate,
)

first_chunk_inputs = processor(
    audio[: processor.num_samples_first_audio_chunk],
    sampling_rate=sampling_rate,
    is_streaming=True,
    is_first_audio_chunk=True,
    return_tensors="pt",
)
first_chunk_inputs = first_chunk_inputs.to(model.device, dtype=model.dtype)


def input_features_generator():
    yield first_chunk_inputs.input_features[:, : processor.num_mel_frames_first_audio_chunk, :]

    mel_frame_idx = processor.num_mel_frames_first_audio_chunk
    hop_length = processor.feature_extractor.hop_length
    n_fft = processor.feature_extractor.n_fft

    start_idx = mel_frame_idx * hop_length - n_fft // 2
    while (end_idx := start_idx + processor.num_samples_per_audio_chunk) < audio.shape[0]:
        inputs = processor(
            audio[start_idx:end_idx],
            sampling_rate=sampling_rate,
            is_streaming=True,
            is_first_audio_chunk=False,
            return_tensors="pt",
        )
        inputs = inputs.to(model.device, dtype=model.dtype)
        yield inputs.input_features

        mel_frame_idx += processor.num_mel_frames_per_audio_chunk
        start_idx = mel_frame_idx * hop_length - n_fft // 2


streamer = TextIteratorStreamer(processor.tokenizer, skip_special_tokens=True)
generate_kwargs = {
    **first_chunk_inputs,
    "input_features": input_features_generator(),
    "streamer": streamer,
}
thread = Thread(target=model.generate, kwargs=generate_kwargs)
thread.start()

print("Model output (streaming):", end=" ", flush=True)
for text_chunk in streamer:
    print(text_chunk, end="", flush=True)
thread.join()
```
</details>

For more details about usage, please refer to the [Transformers documentation](https://huggingface.co/docs/transformers/en/model_doc/nemotron_asr_streaming).


### Input

 * Input Type(s): Audio <br>

 * Input Format(s): wav <br>

 * Input Parameters: One-Dimensional (1D) <br>

 * Other Properties Related to Input: Maximum Length in seconds specific to GPU Memory, No Pre-Processing Needed, Mono channel is required. By leveraging NVIDIA’s hardware (e.g. GPU cores) and software frameworks (e.g., CUDA libraries), the model achieves faster training and inference times compared to CPU-only solutions. <br>

### Output

 * Output Type(s): Text String in English <br>

 * Output Format(s): String <br>

 * Output Parameters: One-Dimensional (1D) <br>

 * Other Properties Related to Output: No Maximum Character Length, transcribe punctuation and capitalization. By leveraging NVIDIA’s hardware (e.g. GPU cores) and software frameworks (e.g., CUDA libraries), the model achieves faster training and inference times compared to CPU-only solutions. <br>


## Try via API — No Setup Required

Transcribe audio using the hosted NVIDIA NIM API on [build.nvidia.com](https://build.nvidia.com/nvidia/nemotron-asr-streaming) — no local GPU, Docker, or model download required.

**1. Get a free API key:** Open [Nemotron ASR Streaming](https://build.nvidia.com/nvidia/nemotron-asr-streaming) and choose **Get API Key**.

**2. Install the Riva client:**

```bash
pip install nvidia-riva-client
```

**3. Transcribe an audio file (offline / whole-file):**

```python
import riva.client

auth = riva.client.Auth(
    uri="grpc.nvcf.nvidia.com:443",
    use_ssl=True,
    metadata_args=[
        ["function-id", "bb0837de-8c7b-481f-9ec8-ef5663e9c1fa"],
        ["authorization", "Bearer nvapi-YOUR_API_KEY"],
    ],
)

asr_service = riva.client.ASRService(auth)

with open("audio.wav", "rb") as f:
    audio = f.read()

config = riva.client.RecognitionConfig(
    language_code="en-US",
    max_alternatives=1,
    enable_automatic_punctuation=True,
    enable_word_time_offsets=True,
)

response = asr_service.offline_recognize(audio, config)
print(response.results[0].alternatives[0].transcript)
```

**Or use the CLI:**

```bash
git clone https://github.com/nvidia-riva/python-clients.git
export NVIDIA_API_KEY="nvapi-YOUR_API_KEY"

python python-clients/scripts/asr/transcribe_file_offline.py \
    --server grpc.nvcf.nvidia.com:443 --use-ssl \
    --metadata function-id "bb0837de-8c7b-481f-9ec8-ef5663e9c1fa" \
    --metadata "authorization" "Bearer $NVIDIA_API_KEY" \
    --language-code en-US \
    --word-time-offsets --automatic-punctuation \
    --input-file audio.wav
```

> **Note:** For streaming, low-latency, and advanced options, use the **API Reference** tab on the [Nemotron ASR Streaming](https://build.nvidia.com/nvidia/nemotron-asr-streaming) model page. The hosted API typically accepts 16-bit mono audio in WAV, OGG, or OPUS; confirm details in the API reference.



## Datasets

### Training Datasets

The majority of the training data comes from NVIDIA Riva ASR training set (250k hours) and the English portion of the Granary dataset [3]:

- YouTube-Commons (YTC) (109.5k hours)
- YODAS2 (102k hours)
- Mosel (14k hours)
- LibriLight (49.5k hours)

In addition, the following datasets were used:

- Librispeech 960 hours
- Fisher Corpus
- Switchboard-1 Dataset
- WSJ-0 and WSJ-1
- National Speech Corpus (Part 1, Part 6)
- VCTK
- VoxPopuli (EN)
- Europarl-ASR (EN)
- Multilingual Librispeech (MLS EN)
- Mozilla Common Voice (v11.0)
- Mozilla Common Voice (v7.0)
- Mozilla Common Voice (v4.0)
- People Speech
- AMI

**Data Modality:** Audio and text

**Audio Training Data Size:** 530k hours

**Data Collection Method:** Human - All audios are human recorded

**Labeling Method:** Hybrid (Human, Synthetic) - Some transcripts are generated by ASR models, while some are manually labeled

### Evaluation Datasets

The model was evaluated on the HuggingFace ASR Leaderboard datasets:

- AMI
- Earnings22
- Gigaspeech
- LibriSpeech test-clean
- LibriSpeech test-other
- SPGI Speech
- TEDLIUM
- VoxPopuli

## Performance

## ASR Performance (w/o PnC)

ASR performance is measured using the Word Error Rate (WER). Both ground-truth and predicted texts are processed using [whisper-normalizer](https://pypi.org/project/whisper-normalizer/) version 0.1.12.

The following tables show the WER on the [HuggingFace OpenASR leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard) datasets:

### Word Error Rate (WER) for chunk size of 1.12s

| | Average | AMI | Earnings22 | Gigaspeech | LS-test-clean | LS-test-other | SPGI | TEDLIUM | VoxPopuli |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **WER (%)** | **6.93** | 11.73 | 12.52 | 9.66 | 2.32 | 4.84 | 2.97 | 3.50 | 7.91 |

### WER for chunk size of 0.56s

| | Average | AMI | Earnings22 | Gigaspeech | LS-test-clean | LS-test-other | SPGI | TEDLIUM | VoxPopuli |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **WER (%)** | **7.07** | 11.88 | 12.82 | 9.78 | 2.46 | 5.07 | 3.03 | 3.54 | 8.00 |

### WER for chunk size of 0.16s

| | Average | AMI | Earnings22 | Gigaspeech | LS-test-clean | LS-test-other | SPGI | TEDLIUM | VoxPopuli |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **WER (%)** | **7.67** | 14.71 | 13.01 | 10.34 | 2.56 | 5.57 | 3.25 | 3.77 | 8.18 |

### WER for chunk size of 0.08s

| | Average | AMI | Earnings22 | Gigaspeech | LS-test-clean | LS-test-other | SPGI | TEDLIUM | VoxPopuli |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **WER (%)** | **8.43** | 18.29 | 13.16 | 11.17 | 2.80 | 6.01 | 3.43 | 4.10 | 8.46 |

## Software Integration

**Runtime Engine:** NeMo 25.11, Riva 2.25.0 or higher

**Supported Hardware Microarchitecture Compatibility:**
- NVIDIA Ampere
- NVIDIA Blackwell
- NVIDIA Hopper
- NVIDIA Volta

**Test Hardware:**
- NVIDIA V100
- NVIDIA A100
- NVIDIA A6000
- DGX Spark

**Preferred/Supported Operating System(s):** Linux

## Ethical Considerations

NVIDIA believes Trustworthy AI is a shared responsibility and we have established policies and practices to enable development for a wide array of AI applications. When downloaded or used in accordance with our terms of service, developers should work with their internal model team to ensure this model meets requirements for the relevant industry and use case and addresses unforeseen product misuse.

Please report model quality, risk, security vulnerabilities or NVIDIA AI Concerns [here](https://www.nvidia.com/en-us/support/submit-security-vulnerability/).

## References

[1] [Stateful Conformer with Cache-based Inference for Streaming Automatic Speech Recognition](https://arxiv.org/abs/2312.17279)

[2] [Fast Conformer with Linearly Scalable Attention for Efficient Speech Recognition](https://arxiv.org/abs/2305.05084)

[3] [NVIDIA Granary](https://huggingface.co/datasets/nvidia/Granary)

[4] [NVIDIA NeMo Framework](https://github.com/NVIDIA/NeMo)