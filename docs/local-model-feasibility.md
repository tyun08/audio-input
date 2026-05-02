# Local / Offline Model — Feasibility Research

This document evaluates the feasibility of replacing the cloud Whisper API with an on-device
`whisper.cpp` inference backend, removing the API round-trip and all audio-privacy concerns.

---

## 1. Why local inference matters

| Cloud (Groq today) | Local (whisper.cpp) |
|--------------------|---------------------|
| Audio leaves the device | Audio never leaves the device |
| ~200–400 ms network round-trip | Zero network latency |
| API cost per minute of audio | Zero marginal cost after one-time download |
| Requires internet connection | Works fully offline |
| Model update is transparent | User must re-download model on update |
| No binary-size overhead | +300 MB – 1.5 GB model files |

For privacy-sensitive workloads (medical notes, company meetings, personal dictation) the local
path is a strong differentiator and, for a future paid tier, a key selling point.

---

## 2. whisper.cpp overview

[whisper.cpp](https://github.com/ggerganov/whisper.cpp) is a C/C++ port of OpenAI Whisper that:

- Runs inference on CPU with optional **Metal** (Apple Silicon) and **CUDA/DirectML** (NVIDIA/AMD) acceleration.
- Produces quantised model files in **GGML** format (`.bin`) that are 3–10× smaller than the
  original PyTorch weights.
- Has a stable C API (`whisper.h`) that can be called from Rust via a thin FFI binding.

### Rust binding options

| Crate | Status | Notes |
|-------|--------|-------|
| [`whisper-rs`](https://crates.io/crates/whisper-rs) | Active, `0.14` | Safe Rust wrapper around `whisper.h`; auto-builds `whisper.cpp` via `cmake`. Supports Metal. |
| Custom `bindgen` | Manual | More control but more maintenance burden. |

**Recommendation:** use `whisper-rs` — it handles `whisper.cpp` compilation and Metal detection
automatically, and the API surface is small.

---

## 3. Model variants and resource requirements

### 3.1 Model family sizes

| Model | Parameters | PyTorch FP16 | GGML q5_0 | GGML q4_0 | Notes |
|-------|-----------|-------------|----------|----------|-------|
| tiny | 39 M | 75 MB | 31 MB | 26 MB | Fastest; accuracy noticeably lower |
| base | 74 M | 142 MB | 57 MB | 50 MB | Good balance for short clips |
| small | 244 M | 466 MB | 182 MB | 158 MB | Recommended minimum for production |
| medium | 769 M | 1.5 GB | 569 MB | 487 MB | Good accuracy; tight on 8 GB RAM |
| large-v3 | 1.55 B | 3.1 GB | 1.18 GB | 1.03 GB | Best accuracy; needs 16 GB |
| **large-v3-turbo** | **809 M** | **1.6 GB** | **~600 MB** | **~520 MB** | **Recommended** — matches Groq Whisper quality |

*large-v3-turbo* is the same model served by the current Groq provider; matching it locally gives
users a seamless quality transition.

### 3.2 RAM requirements

RAM consumed during inference is approximately `2× GGML file size` due to working buffers:

| Model | GGML q5 size | Peak RAM | Apple Silicon unified memory | Verdict |
|-------|-------------|----------|------------------------------|---------|
| tiny | 31 MB | ~80 MB | Any | Dev / testing only |
| small | 182 MB | ~400 MB | Any | Safe for 8 GB |
| medium | 569 MB | ~1.1 GB | 8 GB+ | Fits alongside macOS + app |
| large-v3-turbo | 600 MB | ~1.2 GB | 8 GB+ | **Target** |
| large-v3 | 1.18 GB | ~2.4 GB | 16 GB+ | Premium tier only |

On Apple Silicon (M-series) the CPU and GPU share unified memory.  Running large-v3-turbo
with Metal acceleration requires ~1.2 GB unified memory — well within the 8 GB baseline Mac.

### 3.3 Disk footprint

The model file is downloaded once on first launch and cached in the app data directory
(`~/Library/Application Support/com.audioinput.app/models/`).  No bundling in the `.app` — the
installer stays small.

| Scenario | Download size | Cached on disk |
|----------|--------------|----------------|
| Default (large-v3-turbo q5) | ~600 MB | ~600 MB |
| Fallback (small q5) | ~182 MB | ~182 MB |
| User-selected large-v3 q4 | ~1 GB | ~1 GB |

---

## 4. Metal GPU acceleration (Apple Silicon)

`whisper.cpp` supports Apple's Metal Performance Shaders.  With Metal enabled:

| Clip length | CPU-only (M2 Pro) | Metal (M2 Pro) |
|------------|-------------------|----------------|
| 5 s | ~0.8 s | ~0.2 s |
| 15 s | ~2.5 s | ~0.5 s |
| 30 s | ~5 s | ~1 s |

*Figures from the whisper.cpp benchmark suite; actual times vary by model quantisation.*

`whisper-rs` enables Metal automatically on Apple targets when the `metal` feature flag is set:

```toml
[dependencies]
whisper-rs = { version = "0.14", features = ["metal"] }
```

On Intel Macs and Windows, inference falls back to CPU (still usable for typical dictation clips
of < 30 s).

---

## 5. Integration architecture

The proposed change adds a `"local"` provider alongside the existing `"groq"` and `"vertex_ai"`
entries.  No new IPC commands are needed — the provider pattern is already established.

```
src-tauri/src/
└── transcription/
    ├── groq.rs        (existing)
    ├── vertex.rs      (existing)
    └── local.rs       (new)
```

### 5.1 `local.rs` public API

```rust
pub struct LocalWhisperClient {
    model_path: PathBuf,
}

impl LocalWhisperClient {
    /// Load (or verify) the GGML model file; returns error if not downloaded.
    pub fn new(model_path: PathBuf) -> Result<Self>;

    /// Run inference synchronously on a worker thread.
    pub async fn transcribe(&self, wav_bytes: Vec<u8>) -> Result<String>;
}
```

### 5.2 `commands.rs` addition

```rust
"local" => {
    let model_path = config["model_path"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(default_model_path);
    LocalWhisperClient::new(model_path)?.transcribe(wav_bytes).await
}
```

### 5.3 `providers.ts` addition

```typescript
{
  id: "local",
  name: "Local (whisper.cpp)",
  tagline: l("Offline · Private", "离线 · 完全私密"),
  fields: [
    {
      key: "model_size",
      label: l("Model", "模型"),
      type: "select",
      default: "large-v3-turbo",
      options: [
        { value: "large-v3-turbo", label: "large-v3-turbo (~600 MB, recommended)" },
        { value: "small",          label: "small (~182 MB, faster)" },
        { value: "large-v3",       label: "large-v3 (~1 GB, most accurate)" },
      ],
    },
  ],
  hint: l(
    "Model downloaded once (~600 MB). Audio never leaves your device.",
    "模型一次性下载（约 600 MB），音频不离开设备。",
  ),
}
```

### 5.4 First-launch model download flow

1. User selects **Local** provider in settings.
2. App checks `~/Library/Application Support/.../models/<model>.bin`.
3. If absent: show a progress HUD (`Downloading model… 45%`), stream the file from the
   Hugging Face GGML mirror using `reqwest`.
4. Verify SHA-256 against the published checksum.
5. On completion: mark model as ready and proceed normally.

A `download_model` Tauri command emits `model-download-progress` events (0–100) so the
frontend can render a progress bar without blocking the UI thread.

---

## 6. Windows / Linux considerations

| Platform | Acceleration | Status |
|----------|-------------|--------|
| macOS (Apple Silicon) | Metal | ✅ First target |
| macOS (Intel) | CPU AVX2 | ✅ Supported by whisper.cpp |
| Windows | CPU / CUDA (opt-in) | 🟡 whisper.cpp supports; CUDA requires NVIDIA driver |
| Windows | DirectML | 🟡 Experimental in whisper.cpp |
| Linux | CPU / CUDA (opt-in) | 🟡 No current Linux support in app |

**Recommendation:** ship Metal + CPU-fallback for the first release; add CUDA/DirectML as an
opt-in feature flag behind a build-time `--features cuda` flag.

---

## 7. Binary size and app distribution

| Concern | Mitigation |
|---------|-----------|
| `.app` bundle size inflation | Do **not** bundle the model — download on first use |
| App Store review delay | Homebrew Cask / direct DMG distribution avoids review |
| Model re-download on app update | Cache in `Application Support`; only re-download on model version bump |
| Slow first-run experience | Show clear progress UI; suggest Wi-Fi; allow skipping to cloud provider |

---

## 8. Effort estimate

| Task | Complexity | Notes |
|------|-----------|-------|
| Add `whisper-rs` dependency + Metal feature | Low | ~1 day; whisper.cpp compiles automatically |
| Implement `local.rs` transcription module | Medium | ~2 days; mirror `groq.rs` structure |
| Model download command + progress events | Medium | ~2 days |
| Settings UI (provider option + download button) | Low | ~1 day |
| SHA-256 checksum verification | Low | ~0.5 day |
| Integration tests (mock GGML) | Medium | ~1 day |
| **Total** | | **~7–8 days** |

---

## 9. Recommendation

**Proceed with local inference for v0.5.0.**

The `whisper-rs` crate makes the integration straightforward.  The primary challenge is the
first-launch model download UX, not the inference code itself.  Start with `large-v3-turbo q5`
(~600 MB) on Apple Silicon with Metal acceleration — this matches the current Groq quality and
delivers sub-500 ms latency on M-series hardware.

See [`local-model-fine-tuning.md`](./local-model-fine-tuning.md) for the personalisation and
fine-tuning layer that makes accuracy improve over time.
