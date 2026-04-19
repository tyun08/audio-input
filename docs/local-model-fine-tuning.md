# Local Model Fine-Tuning — Research

This document explores how to make the on-device Whisper model continuously improve on a
specific user's voice, accent, and vocabulary over time — without ever sending audio to a server.

---

## 1. What "fine-tuning" means for a speech model

Whisper is a sequence-to-sequence Transformer: it maps a log-Mel spectrogram (audio) to a
token sequence (text).  Fine-tuning adjusts the model's weights so that the mapping improves
on a target distribution (your voice, your vocabulary, your speaking style) while retaining
general ability on other audio.

Two levels of adaptation exist:

| Level | What changes | Data needed | Compute |
|-------|-------------|-------------|---------|
| **Prompt / vocabulary hint** | Nothing — model weights unchanged | None | None |
| **LoRA / adapter fine-tuning** | Small adapter layers added; base weights frozen | 10–100 labeled clips | CPU feasible; GPU faster |
| **Full fine-tuning** | All weights updated | 1 000+ clips | GPU required |

For a single-user desktop app, **LoRA adapter fine-tuning** is the only practical approach.
Full fine-tuning requires a GPU-equipped machine and large datasets, and is the domain of model
labs, not end-user devices.

---

## 2. How LoRA adapters work

Low-Rank Adaptation (LoRA) inserts two small weight matrices (**A** and **B**) into each
attention layer.  Only A and B are updated during training; the frozen base model is never
modified.

```
                 ┌─────────┐
Input ─────────► │  W_base │ ──────────────────────►  +  ──► Output
                 │ (frozen)│          ┌────────────────┘
                 └─────────┘          │
                                  A · B  (trainable; rank r ≪ d)
```

- **Base model:** large-v3-turbo (~1.5 B parameters)
- **LoRA rank r = 8:** adds ~2 M trainable parameters — roughly 0.1% of the total
- **Adapter file size on disk:** ~8 MB per adapter checkpoint
- **Training on M2 Pro (CPU):** ~30 s per epoch on 50 clips; GPU cuts this 5–10×

---

## 3. Training data requirements

### 3.1 Minimum viable dataset

| Goal | Clips needed | Duration | Labelling effort |
|------|-------------|---------|-----------------|
| Accent / pronunciation adaptation | 20–50 clips | 5–15 min total | High — each clip needs a correct transcript |
| Domain vocabulary (< 200 words) | 30–80 clips | 8–20 min total | High |
| Combined accent + vocabulary | 50–150 clips | 15–40 min total | High |

In practice **50 labeled clips (≈ 10 minutes of audio)** is the minimum for measurable
improvement.  Below this threshold the model may overfit, losing general accuracy.

### 3.2 Collecting training data in-app

The app already captures every recording as a WAV buffer.  Two data collection paths:

**Path A — explicit correction loop (recommended for first release)**

1. User records and gets transcription.
2. A small "correct this" button appears next to the transcribed text.
3. User edits the text inline to the true transcript.
4. The `(wav_bytes, corrected_text)` pair is appended to a local SQLite table
   (`corrections` table: `id, recorded_at, wav_path, raw_transcript, corrected_transcript`).
5. When ≥ 50 corrections exist, a "Train model" button activates.

**Path B — passive collection (future)**

- Use a high-confidence threshold on the existing Groq/cloud transcript.
- Auto-label high-confidence clips as silver training data (auto-labeled).
- These serve as "silver" training data — lower quality, but requires zero user effort.

### 3.3 Audio quality requirements

| Parameter | Requirement | Why |
|-----------|-------------|-----|
| Sample rate | 16 kHz (already used) | Whisper's expected input |
| Channels | Mono (already used) | Stereo degrades fine-tuning |
| Clip length | 3–30 s | Whisper's context window is 30 s |
| SNR (signal-to-noise ratio) | > 15 dB | Background noise corrupts label-audio alignment |
| Microphone consistency | Same device preferred | Different mics introduce distribution shift |

---

## 4. Loss function

Whisper is trained with **cross-entropy loss** over the output token sequence (standard
sequence-to-sequence objective):

```
L = - (1/T) Σ_{t=1}^{T} log P(y_t | y_{<t}, audio)
```

where `y_t` is the ground-truth token at step `t` and `T` is the total number of tokens in
the reference transcript.

During fine-tuning the same loss is used.  Two common modifications for small datasets:

### 4.1 Label smoothing

Replace hard one-hot targets with a soft distribution (ε = 0.1):

```
L_smooth = (1 - ε) · L_CE  +  ε · (uniform over vocabulary)
```

**Effect:** prevents the model from being over-confident on the small correction set; improves
generalisation to out-of-vocabulary words.

### 4.2 CTC auxiliary loss (optional)

Connectionist Temporal Classification loss can be added as a regulariser when alignment is
uncertain.  Not recommended for the first implementation — standard cross-entropy with label
smoothing is sufficient.

### 4.3 Monitoring loss during training

A healthy fine-tuning run on 50 clips (80/20 train/validation split) should show:

| Epoch | Train loss | Val loss | WER delta |
|-------|-----------|---------|-----------|
| 0 | baseline | baseline | 0% |
| 1 | ↓ 15–25% | ↓ 10–20% | -2 to -5% |
| 3 | ↓ 30–40% | ↓ 20–30% | -5 to -15% |
| 10 | plateau | begins ↑ | overfitting risk |

**Early stopping criterion:** stop when validation loss increases for 2 consecutive epochs.
Save the checkpoint at minimum validation loss.

### 4.4 Word Error Rate (WER) — the real metric

WER = (Substitutions + Deletions + Insertions) / Reference word count

Track WER on a held-out set of 10 clips that were never used for training.  A fine-tuned
adapter is only deployed if `WER_fine_tuned < WER_base` on this eval set.

---

## 5. Training pipeline design

### 5.1 Components

```
Corrections DB (SQLite)
       │
       ▼
 Feature extractor          ← converts WAV → log-Mel spectrogram
       │
       ▼
 LoRA fine-tuning job       ← Python script OR in-process Rust/C
       │
       ▼
 Adapter checkpoint (.bin)  ← merged with whisper.cpp GGML base
       │
       ▼
 Eval on held-out set        ← WER check; reject if no improvement
       │
       ▼
 Hot-swap adapter            ← next recording uses updated model
```

### 5.2 Implementation options

| Option | Language | Pros | Cons |
|--------|---------|------|------|
| Python subprocess (first release) | Python | Mature HuggingFace / PEFT ecosystem; easy to prototype | Requires Python on user machine |
| Candle (Rust) | Rust | No external runtime; native | Less mature than PyTorch for Whisper fine-tuning |
| PyTorch C++ LibTorch | C++ | Native; no Python | Complex build |

**Recommendation for first release:** ship a bundled Python script that uses
`transformers` + `peft` (for LoRA) + `datasets`. The Tauri app launches it as a sidecar process
(`tauri-plugin-shell`) with stdin/stdout IPC for progress reporting.  A future release can port
the training loop to Candle for a fully self-contained binary.

### 5.3 Approximate compute requirements for training

| Hardware | Time for 50 clips × 3 epochs | Notes |
|----------|------------------------------|-------|
| M2 Pro (CPU, no Metal) | ~5 min | Acceptable for background job |
| M2 Pro (MPS / Metal) | ~1 min | Excellent; triggered silently |
| Intel Mac (CPU) | ~15–25 min | Acceptable if triggered overnight |
| Windows CPU (no CUDA) | ~20–30 min | Background job |
| Windows + NVIDIA GPU | ~1–2 min | Ideal |

Training can run as a background job triggered when:
- The device is idle (no recording in progress)
- Battery > 30% (or plugged in on laptop)
- ≥ 50 new corrections have accumulated since the last training run

---

## 6. Continuous improvement loop

```
┌──────────────────────────────────────────────────────────────────┐
│                       User interaction                            │
│                                                                  │
│  Record → Transcribe (local) → [Optional: correct transcript]    │
│                │                           │                     │
│        raw_text stored               correction stored           │
│                                            │                     │
└────────────────────────────────────────────┼─────────────────────┘
                                             ▼
                             Corrections DB (SQLite)
                                             │
                              ┌──────────────┴────────────────┐
                              │  Background fine-tuning job   │
                              │  (triggered when ≥ 50 new)    │
                              └──────────────┬────────────────┘
                                             │
                              ┌──────────────▼────────────────┐
                              │  WER eval on held-out set     │
                              │  Accept if WER improves       │
                              └──────────────┬────────────────┘
                                             │
                              ┌──────────────▼────────────────┐
                              │  Hot-swap adapter checkpoint  │
                              │  (next recording uses it)     │
                              └───────────────────────────────┘
```

Key properties of this loop:

- **Incremental:** new corrections are added to existing training data; adapter is retrained
  from the previous checkpoint (warm-start), not from scratch.
- **Safe:** WER gate ensures quality never regresses.
- **Reversible:** user can revert to the base model checkpoint at any time.
- **Private:** no data, audio, or corrections ever leave the device.

### 6.1 Data versioning

Each training run produces a versioned adapter checkpoint:
```
~/Library/Application Support/.../adapters/
├── adapter-v0.bin    (base — no adaptation)
├── adapter-v1.bin    (after first 50 corrections)
├── adapter-v2.bin    (after next 50 corrections)
└── active -> adapter-v2.bin   (symlink to current)
```

Rolling back to `adapter-v1.bin` is a single symlink update.

---

## 7. Privacy and security

| Concern | Mitigation |
|---------|-----------|
| Correction data contains sensitive speech | Stored only in local SQLite; never uploaded |
| Model adapter leaks user voice | Adapter is local; not synced to cloud |
| Malicious model replacement | SHA-256 of base GGML verified on every load |
| Adapter poisoning via crafted audio | Only corrections from the local user are used as training data |

---

## 8. RAM and disk budget for the full local stack

Assuming large-v3-turbo as the base model, with fine-tuning enabled:

| Component | Disk | Peak RAM |
|-----------|------|---------|
| GGML base model (q5) | 600 MB | ~1.2 GB |
| Active LoRA adapter | ~8 MB | ~16 MB overhead |
| SQLite corrections DB (1 000 clips × 30 s) | ~5 GB WAV or ~250 MB MP3 | — |
| Python training sidecar (peak, during fine-tune) | — | ~2.5 GB additional |

The fine-tuning job runs only when the app is idle.  During normal transcription (no training),
the total RAM overhead of local inference is ~1.2 GB — well within the 8 GB baseline Mac.

If the user chooses to store raw WAV corrections, consider converting them to **FLAC** (lossless,
~2–3× compression for 16 kHz mono speech) or **Opus at 24 kbps** (lossy, ~30–60× compression
for speech; ratios are lower than for music because speech has narrower bandwidth and longer
silences). Both preserve enough fidelity for Whisper fine-tuning.

---

## 9. Open questions and risks

| Question | Risk level | Notes |
|----------|-----------|-------|
| Does `whisper-rs` support loading LoRA adapters? | Medium | whisper.cpp LoRA support is experimental; may need to use a separate Python fine-tuning path and re-merge weights into a new GGML file. **Action item before Phase 3:** prototype loading a merged LoRA+base GGML with `whisper-rs` and confirm the API surface; if unsupported, the Python sidecar path in section 5.2 is the fallback |
| How many corrections until improvement is user-visible? | Medium | Depends strongly on accent diversity in pre-training data; 50 clips is a reasonable lower bound |
| Will LoRA adapter transfer across base model versions? | High | Adapters are tightly coupled to model architecture; a model update invalidates existing adapters |
| Is Python sidecar acceptable for distribution? | Medium | Homebrew / direct DMG: yes (ship a bundled Python). App Store: complicated — may need Candle |
| Does the WER gate work without human reference transcripts? | High | Automatic WER requires verified transcripts on the eval set; need explicit user-confirmed corrections for the eval split |

---

## 10. Recommended implementation phases

### Phase 1 — Baseline local inference (no fine-tuning)
*(See `local-model-feasibility.md`)*

Ship whisper.cpp integration with `whisper-rs`, Metal acceleration, and model download UX.
Establishes the infrastructure needed for Phase 2.

### Phase 2 — Correction collection
- Add "correct this" UI to the HUD (shows after each transcription).
- Store `(wav_path, corrected_transcript)` pairs in SQLite.
- Show correction count in settings.
- No training yet — just accumulating data.

### Phase 3 — On-device fine-tuning
- Bundle Python sidecar (or Candle when ready) for training.
- Trigger background fine-tuning when ≥ 50 new corrections exist.
- WER evaluation gate before deploying new adapter.
- Versioned adapter checkpoints with rollback UI.

### Phase 4 — Automated passive collection
- Auto-label high-confidence clips as silver training data.
- Tune confidence threshold to balance data volume vs. label noise.
- Monitor WER trend over time; alert user if quality plateaus.
