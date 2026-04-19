# Transcription Accuracy Improvement — Approaches & Trade-offs

This document explores every practical technique available to improve the accuracy of speech-to-text transcription in Audio Input over time. Each approach is evaluated on feasibility, cost, latency impact, durability (works session after session), and scalability (works regardless of how many users or domains).

---

## Current Pipeline (baseline)

```
Microphone → WAV encoder → Whisper API (Groq / Vertex AI Gemini)
           → (optional) LLM polish → text injection
```

Known weaknesses of the baseline:
- Whisper hallucinates on silence or very short clips.
- Technical terms, proper nouns, and non-English brand names are frequently mis-transcribed.
- The polish LLM has no memory between sessions — it cannot "learn" from corrections.

---

## Approach 1 — Whisper `prompt` Parameter (static vocabulary hints)

**Status: implemented** (`groq.rs`, `config.rs`)

The Whisper API accepts an optional `prompt` string that is prepended to the decoding context. Providing a comma-separated list of known words biases the model to spell them correctly.

**How it works:**  
User fills in a "Custom Vocabulary" list in Settings → Advanced. Each word is joined and sent as the `prompt` field on every transcription request.

| | |
|---|---|
| **Pros** | Zero latency overhead; free; works on the very first use; persists across sessions via config; deterministic (same words always boosted) |
| **Cons** | Static — user must manually maintain the list; prompt length is limited (~224 tokens); no feedback loop, so new words are never discovered automatically; helps only with spelling, not grammar |

---

## Approach 2 — LLM Polish with Vocabulary Injection

**Status: implemented** (`polish.rs`, `vertex.rs`)

The same custom vocabulary list is appended to the system prompt of the post-processing LLM. The LLM is explicitly told "these words may appear" and uses that context when fixing homophones and mis-hearings.

**How it works:**  
`build_system_prompt_text(vocabulary)` and `build_system_prompt_vision(vocabulary)` append a "Known words/terms" sentence to the relevant system prompt before each polish call.

| | |
|---|---|
| **Pros** | Combines Whisper-level bias + LLM-level correction; the LLM can handle context that pure Whisper cannot (e.g., infer from surrounding words); no extra API call |
| **Cons** | Same static limitation as Approach 1; LLM can still hallucinate a correction; adds prompt tokens, slightly increasing polish latency and cost |

---

## Approach 3 — Screenshot / Visual Context (screen-grounded correction)

**Status: implemented** (`screenshot.rs`, `polish.rs` vision path)

At the start of each recording a screenshot is captured and sent alongside the transcription to a multimodal LLM (llama-4-scout / Gemini). The LLM reads visible text on screen (code, documentation, UI labels) and uses it as reference when resolving ambiguous words.

**How it works:**  
Toggle "Screenshot Context" in Settings → Advanced. The vision model is tried first; if it fails or returns too-short output, it falls back to the text-only model.

| | |
|---|---|
| **Pros** | Fully automatic — no manual vocabulary maintenance; captures ephemeral context (what the user is actually looking at); works for any domain the screen shows; no storage required |
| **Cons** | Requires a vision-capable model (higher cost, ~8 s timeout vs 3 s for text-only); screenshot capture can fail on locked screens or certain display configurations; privacy-sensitive (screenshot is sent to the LLM provider); context is only as good as what is visible |

---

## Approach 4 — Accumulated Word Learning from Past Transcriptions

**Status: not implemented — proposed**

After each successful transcription, extract low-frequency or domain-specific tokens and add them to a persistent local vocabulary store. The store grows over time and is fed back into the Whisper prompt and LLM system prompt on future recordings.

**How it works (proposed):**  
1. After polish, diff the raw Whisper output against the polished text.  
2. Words that were changed by the LLM (i.e., corrections) are candidates for the vocabulary store.  
3. If the same correction appears ≥ N times, promote it to the vocabulary list automatically.  
4. Optionally expose a UI to review and approve/reject suggested additions.

| | |
|---|---|
| **Pros** | Self-improving — accuracy increases passively with usage; no manual effort once tuned; captures user-specific jargon, names, and project terms automatically |
| **Cons** | Requires persistent storage and diffing logic; false positives (wrong corrections get promoted); the diff must distinguish "real vocabulary" from punctuation/grammar edits; cold-start problem (needs enough volume to accumulate reliable signal); could balloon the vocabulary list and hit Whisper's prompt token limit |

---

## Approach 5 — User Correction Feedback Loop

**Status: not implemented — proposed**

Show the user a brief editable preview of the transcribed text before injection. If the user edits it, record the (original → corrected) pair and use it to update the vocabulary or fine-tune prompts.

**How it works (proposed):**  
1. After transcription, instead of injecting immediately, show a small HUD with the text and a 2–3 s cancel/edit window.  
2. If the user edits the text, store the diff as a correction pair.  
3. Periodically consolidate correction pairs into vocabulary additions or polish prompt examples (few-shot).

| | |
|---|---|
| **Pros** | Highest-quality signal — corrections are ground-truth; enables few-shot examples in the polish prompt; allows per-user personalisation |
| **Cons** | Breaks the instant-injection UX (the key selling point); even a short delay feels intrusive; implementation complexity is high; most users will not edit even when wrong; requires carefully designed UI to avoid friction |

---

## Approach 6 — Forced Language Specification

**Status: not implemented — mentioned in roadmap**

Whisper auto-detects the language of each recording but occasionally mis-identifies it (e.g., English with technical acronyms is flagged as another language). Passing an explicit `language` parameter eliminates this source of error.

**How it works (proposed):**  
Add a "Language" dropdown to Settings → Transcription. Pass the selected ISO 639-1 code as the `language` field in the Groq multipart form and as a constraint in the Vertex AI prompt.

| | |
|---|---|
| **Pros** | Eliminates language-detection errors entirely; zero cost and latency overhead; simple to implement |
| **Cons** | Breaks multilingual use cases (user must switch manually); no benefit for single-language users who already get correct detection |

---

## Approach 7 — Model Selection / Upgrade

**Status: partially implemented** (model selector in provider config)

Using a larger or more accurate model increases baseline accuracy independent of any prompting.

**Groq options:**
| Model | Speed | Cost | Accuracy |
|---|---|---|---|
| `whisper-large-v3-turbo` (default) | Fastest | $0.04/hr | Good |
| `whisper-large-v3` | 2× slower | $0.11/hr | Best Whisper accuracy |

**Vertex AI:**  
Gemini 2.5 Pro is more accurate than 2.5 Flash for ambiguous audio but ~3× more expensive and slower.

| | |
|---|---|
| **Pros** | Immediate accuracy gain with zero code changes; user-controllable trade-off between speed and quality |
| **Cons** | Cost scales with usage; larger models have higher latency; accuracy ceiling is still set by Whisper architecture |

---

## Approach 8 — Local / Offline Model (whisper.cpp)

**Status: not implemented — mentioned in roadmap as P2**

Running Whisper locally via `whisper.cpp` eliminates the API round-trip and removes privacy concerns. The local model can be fine-tuned on user data.

**How it works (proposed):**  
Integrate a `whisper.cpp` binding (e.g., the `whisper-rs` crate) and add a "Local" provider option. Audio is processed entirely on-device.

| | |
|---|---|
| **Pros** | Works offline; zero API cost after download; no audio leaves the device; can be fine-tuned on user-specific audio; lowest latency for short clips on Apple Silicon |
| **Cons** | Large binary size (model files are 300 MB – 1.5 GB); requires significant implementation effort; fine-tuning requires labelled data and GPU; model must be re-downloaded when updated; accuracy on CPU is slower than Groq cloud inference |

---

## Approach 9 — Post-processing Rules Engine

**Status: not implemented — proposed**

A deterministic, user-maintained substitution table that runs after transcription and before polish. For example, always replace "jen's" → "Jens", or "react query" → "React Query".

**How it works (proposed):**  
Store a list of `(pattern, replacement)` pairs in config. Apply them in order using simple string or regex matching on the raw Whisper output, before the LLM polish step.

| | |
|---|---|
| **Pros** | 100% deterministic and predictable; no latency overhead; no API cost; handles cases the LLM gets wrong consistently; easy for the user to maintain |
| **Cons** | Manual maintenance; order-sensitive (can produce unintended matches); case sensitivity is tricky; does not generalise — each substitution must be defined explicitly |

---

## Approach 10 — Audio Pre-processing (noise reduction, normalisation)

**Status: not implemented — proposed**

Improving the audio quality before it reaches Whisper can reduce transcription errors, especially in noisy environments.

**How it works (proposed):**  
Apply one or more of: silence trimming, noise gate, RMS normalisation, or a lightweight RNNoise pass to the WAV bytes before sending them to the API.

| | |
|---|---|
| **Pros** | Helps all downstream models equally; no API changes; can prevent the "silence hallucination" problem (Whisper inventing text from background noise) |
| **Cons** | Aggressive noise reduction can distort speech; adds CPU overhead (blocking on the audio thread); silence detection is already implemented and handles the most common case |

---

## Summary Matrix

| # | Approach | Effort | Latency impact | Cost impact | Self-improving? | Status |
|---|---|---|---|---|---|---|
| 1 | Whisper `prompt` (vocabulary) | Low | None | None | No | ✅ Done |
| 2 | LLM polish vocabulary injection | Low | Minimal | Minimal | No | ✅ Done |
| 3 | Screenshot / visual context | Medium | +2–5 s (vision path) | +LLM call | No | ✅ Done |
| 4 | Accumulated word learning | High | None | None | **Yes** | 🔲 Proposed |
| 5 | User correction feedback loop | High | +2–3 s UX delay | Minimal | **Yes** | 🔲 Proposed |
| 6 | Forced language | Low | None | None | No | 🔲 Roadmap |
| 7 | Model upgrade | Low | +latency | +cost | No | ✅ Partial |
| 8 | Local whisper.cpp | Very High | Lower for short clips | None after setup | Optional (fine-tune) | 🔲 Roadmap P2 |
| 9 | Post-processing rules engine | Low | None | None | No | 🔲 Proposed |
| 10 | Audio pre-processing | Medium | +CPU | None | No | 🔲 Proposed |

---

## Recommended Priority

1. **Approach 4 (accumulated word learning)** — highest long-term impact for zero ongoing user effort. Combine with Approach 1: automatically promote frequently-corrected words into the vocabulary list.
2. **Approach 6 (forced language)** — tiny effort, eliminates a whole class of detection errors.
3. **Approach 9 (rules engine)** — complements the LLM polish for names and brand spellings the model consistently gets wrong.
4. **Approach 5 (user corrections)** — consider only if UX disruption can be minimised (e.g., inline correction in the last-result tray menu item, not a blocking prompt).
5. **Approach 10 (audio pre-processing)** — lower priority because silence detection already handles the main hallucination case; revisit if users report noise issues.
6. **Approach 8 (local model)** — valuable for privacy-first users but high implementation cost; defer to a dedicated milestone.
