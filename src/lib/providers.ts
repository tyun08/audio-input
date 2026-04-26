/**
 * Provider registry — the single source of truth for all LLM providers.
 *
 * To add a new provider:
 *   1. Add an entry to `providers` below (frontend metadata + fields).
 *   2. Create `src-tauri/src/transcription/<id>.rs` (transcribe + polish).
 *   3. Add a match arm in `commands.rs` → `transcribe_with_provider` / `polish_with_provider`.
 *   That's it — no new IPC commands, no config schema changes.
 */

import { type L, l } from "./i18n";

export interface ProviderField {
  key: string;
  label: L;
  type: "text" | "password" | "select";
  placeholder?: string;
  options?: { value: string; label: string }[];
  default?: string;
  mono?: boolean;
  /** If true, this field and the next `half` field share one row. */
  half?: boolean;
}

export interface ProviderDef {
  id: string;
  name: string;
  tagline: L;
  icon: string; // SVG inner content for a 24×24 viewBox
  fields: ProviderField[];
  /** IPC command that returns boolean — called with `{ provider: id }` */
  authCheck?: string;
  authOkText?: L;
  authFailText?: L;
  /** HTML string shown below the config form */
  hint?: L;
}

export const providers: ProviderDef[] = [
  {
    id: "qwen3_asr",
    name: "Qwen3-ASR",
    tagline: l("Local · Streaming · No internet", "本地推理 · 流式输出 · 无需联网"),
    icon: '<rect x="2" y="3" width="20" height="14" rx="2" stroke="currentColor" stroke-width="1.8"/><path d="M8 21h8M12 17v4" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/><path d="M7 8v3a5 5 0 0 0 10 0V8" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/><line x1="12" y1="3" x2="12" y2="7" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>',
    fields: [
      {
        key: "model",
        label: l("Model", "模型"),
        type: "select",
        default: "Qwen/Qwen3-ASR-0.6B",
        half: true,
        options: [
          { value: "Qwen/Qwen3-ASR-0.6B", label: "Qwen3-ASR-0.6B (fast)" },
          { value: "Qwen/Qwen3-ASR-1.7B", label: "Qwen3-ASR-1.7B (accurate)" },
        ],
      },
      {
        key: "language",
        label: l("Language", "语言"),
        type: "select",
        default: "auto",
        half: true,
        options: [
          { value: "auto", label: "Auto-detect" },
          { value: "Chinese", label: "中文 Chinese" },
          { value: "English", label: "English" },
          { value: "Japanese", label: "日本語 Japanese" },
          { value: "Korean", label: "한국어 Korean" },
          { value: "French", label: "Français French" },
          { value: "German", label: "Deutsch German" },
          { value: "Spanish", label: "Español Spanish" },
        ],
      },
      {
        key: "host",
        label: l("Server Host", "服务器地址"),
        type: "text",
        placeholder: "localhost",
        mono: true,
        half: true,
      },
      {
        key: "port",
        label: l("Port", "端口"),
        type: "text",
        placeholder: "8000",
        mono: true,
        half: true,
      },
      {
        key: "api_key",
        label: l("API Key (optional)", "API Key（可选）"),
        type: "password",
        placeholder: l("Leave blank if server has no auth", "无鉴权可留空").en,
      },
    ],
    authCheck: "check_qwen3_asr_status",
    authOkText: l("vLLM server reachable · ready", "vLLM 服务运行中 · 已就绪"),
    authFailText: l("Cannot reach vLLM server", "无法连接 vLLM 服务"),
    hint: l(
      'Install: <code>pip install qwen-asr torch flask</code><br>Start: <code>python scripts/qwen3_asr_server.py</code>',
      '安装: <code>pip install qwen-asr torch flask</code><br>启动: <code>python scripts/qwen3_asr_server.py</code>'
    ),
  },
  {
    id: "groq",
    name: "Groq",
    tagline: l("Free API Key", "免费 API Key"),
    icon: '<path d="M13 2L3 14h9l-1 8 10-12h-9l1-8z" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>',
    fields: [
      {
        key: "api_key",
        label: l("API Key", "API Key"),
        type: "password",
        placeholder: "gsk_...",
      },
      {
        key: "model",
        label: l("Model", "模型"),
        type: "select",
        default: "whisper-large-v3-turbo",
        options: [
          { value: "whisper-large-v3-turbo", label: "Whisper Large v3 Turbo (fast · cheap)" },
          { value: "whisper-large-v3", label: "Whisper Large v3 (accurate · 3× cost)" },
        ],
      },
    ],
    hint: l(
      'Get one free at <a href="https://console.groq.com" target="_blank" rel="noopener">console.groq.com</a>',
      '在 <a href="https://console.groq.com" target="_blank" rel="noopener">console.groq.com</a> 免费获取'
    ),
  },
  {
    id: "vertex_ai",
    name: "Vertex AI",
    tagline: l("Google Cloud", "Google Cloud"),
    icon: '<path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"/><polyline points="3.27 6.96 12 12.01 20.73 6.96" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/><line x1="12" y1="22.08" x2="12" y2="12" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>',
    fields: [
      {
        key: "project_id",
        label: l("GCP Project ID", "GCP 项目 ID"),
        type: "text",
        placeholder: "my-project-id",
      },
      {
        key: "location",
        label: l("Region", "区域"),
        type: "text",
        placeholder: "us-central1",
        mono: true,
        half: true,
      },
      {
        key: "model",
        label: l("Model", "模型"),
        type: "select",
        half: true,
        options: [
          { value: "gemini-2.5-flash", label: "Gemini 2.5 Flash" },
          { value: "gemini-2.5-pro", label: "Gemini 2.5 Pro" },
          { value: "gemini-2.0-flash", label: "Gemini 2.0 Flash" },
        ],
      },
    ],
    authCheck: "check_provider_status",
    authOkText: l("gcloud credentials ready", "gcloud 凭证已就绪"),
    authFailText: l("gcloud credentials not found", "未检测到 gcloud 凭证"),
    hint: l(
      "Run <code>gcloud auth application-default login</code>",
      "请运行 <code>gcloud auth application-default login</code>"
    ),
  },
];

export function getProvider(id: string): ProviderDef | undefined {
  return providers.find((p) => p.id === id);
}

/** Group fields so adjacent `half` fields share one row. */
export function groupFields(fields: ProviderField[]): ProviderField[][] {
  const groups: ProviderField[][] = [];
  let i = 0;
  while (i < fields.length) {
    if (fields[i].half && i + 1 < fields.length && fields[i + 1].half) {
      groups.push([fields[i], fields[i + 1]]);
      i += 2;
    } else {
      groups.push([fields[i]]);
      i += 1;
    }
  }
  return groups;
}
