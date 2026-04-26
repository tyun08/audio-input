#!/usr/bin/env python3
"""
Qwen3-ASR local server — Mac-compatible (Apple Silicon MPS or CPU).
Exposes an OpenAI-compatible /v1/chat/completions endpoint with SSE streaming.

Install:
    pip install qwen-asr torch flask

Run:
    python scripts/qwen3_asr_server.py
    python scripts/qwen3_asr_server.py --model Qwen/Qwen3-ASR-1.7B --port 8000
    python scripts/qwen3_asr_server.py --api-key mysecretkey
"""

import argparse
import base64
import json
import os
import re
import tempfile
import time
import uuid

import torch
from flask import Flask, Response, jsonify, request, stream_with_context
from qwen_asr import Qwen3ASRModel

# ---------------------------------------------------------------------------
# Globals
# ---------------------------------------------------------------------------

app = Flask(__name__)
model: Qwen3ASRModel | None = None
model_name: str = "Qwen/Qwen3-ASR-0.6B"
server_api_key: str | None = None


def _device() -> str:
    if torch.backends.mps.is_available():
        return "mps"
    if torch.cuda.is_available():
        return "cuda:0"
    return "cpu"


def _dtype():
    # bfloat16 works on MPS (M1+) and CUDA; use float32 on CPU
    device = _device()
    if device in ("mps", "cuda:0"):
        return torch.bfloat16
    return torch.float32


def load_model(name: str) -> None:
    global model, model_name
    model_name = name
    device = _device()
    dtype = _dtype()
    print(f"[qwen3-asr-server] Loading {name} on {device} ({dtype}) …")
    model = Qwen3ASRModel.from_pretrained(
        name,
        dtype=dtype,
        device_map=device,
        max_new_tokens=512,
    )
    print("[qwen3-asr-server] Model ready.")


# ---------------------------------------------------------------------------
# Auth middleware
# ---------------------------------------------------------------------------

def _check_auth() -> bool:
    if not server_api_key:
        return True
    auth = request.headers.get("Authorization", "")
    return auth == f"Bearer {server_api_key}"


# ---------------------------------------------------------------------------
# Routes
# ---------------------------------------------------------------------------

@app.get("/health")
def health():
    return jsonify({"status": "ok"})


@app.post("/v1/chat/completions")
def chat_completions():
    if not _check_auth():
        return jsonify({"error": {"message": "API key required", "type": "authentication_error"}}), 401

    body = request.get_json(force=True)
    is_stream = body.get("stream", False)

    # --- Extract audio bytes + language from messages ---
    audio_bytes: bytes | None = None
    language: str | None = None

    for msg in body.get("messages", []):
        role = msg.get("role", "")
        content = msg.get("content", "")

        if role == "system" and isinstance(content, str):
            m = re.search(r"in ([A-Za-z]+)\.", content)
            if m:
                language = m.group(1)

        elif role == "user" and isinstance(content, list):
            for part in content:
                if part.get("type") == "audio_url":
                    url = part["audio_url"]["url"]
                    if url.startswith("data:"):
                        _, b64 = url.split(",", 1)
                        audio_bytes = base64.b64decode(b64)

    if audio_bytes is None:
        return jsonify({"error": {"message": "No audio_url found in request"}}), 400

    # --- Write to temp file and transcribe ---
    with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as f:
        f.write(audio_bytes)
        tmp_path = f.name

    try:
        results = model.transcribe(audio=tmp_path, language=language)
        text: str = results[0].text.strip() if results else ""
    except Exception as e:
        return jsonify({"error": {"message": str(e)}}), 500
    finally:
        os.unlink(tmp_path)

    # --- Build response ---
    completion_id = f"chatcmpl-{uuid.uuid4().hex[:10]}"
    created = int(time.time())
    req_model = body.get("model", model_name)

    if is_stream:
        def generate():
            # Stream word-by-word so the UI shows live output.
            # A small sleep between tokens is required — without it all SSE
            # chunks arrive in the same millisecond and the client receives
            # them as one batch, defeating the streaming effect.
            words = text.split(" ")
            for i, word in enumerate(words):
                token = word if i == 0 else " " + word
                chunk = {
                    "id": completion_id,
                    "object": "chat.completion.chunk",
                    "created": created,
                    "model": req_model,
                    "choices": [
                        {"index": 0, "delta": {"content": token}, "finish_reason": None}
                    ],
                }
                yield f"data: {json.dumps(chunk, ensure_ascii=False)}\n\n"
                time.sleep(0.04)  # ~25 words/sec — perceptible streaming rate
            yield "data: [DONE]\n\n"

        return Response(stream_with_context(generate()), mimetype="text/event-stream",
                        headers={"X-Accel-Buffering": "no", "Cache-Control": "no-cache",
                                 "Transfer-Encoding": "chunked"})

    # Non-streaming fallback
    return jsonify({
        "id": completion_id,
        "object": "chat.completion",
        "created": created,
        "model": req_model,
        "choices": [
            {"index": 0, "message": {"role": "assistant", "content": text}, "finish_reason": "stop"}
        ],
    })


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Qwen3-ASR local server")
    parser.add_argument("--model", default="Qwen/Qwen3-ASR-0.6B",
                        choices=["Qwen/Qwen3-ASR-0.6B", "Qwen/Qwen3-ASR-1.7B"])
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8000)
    parser.add_argument("--api-key", default=None,
                        help="Require this key in Authorization: Bearer header")
    args = parser.parse_args()

    server_api_key = args.api_key
    load_model(args.model)

    print(f"[qwen3-asr-server] Listening on http://{args.host}:{args.port}")
    # use_reloader=False is important — reloader forks the process and loads the model twice
    app.run(host=args.host, port=args.port, threaded=False, use_reloader=False)
