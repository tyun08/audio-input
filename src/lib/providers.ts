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
  {
    id: "litellm",
    name: "LiteLLM",
    tagline: l("OpenAI, Gemini, Groq & more", "OpenAI、Gemini、Groq 等"),
    icon: '<circle cx="12" cy="12" r="3" stroke="currentColor" stroke-width="1.8"/><path d="M12 2v3M12 19v3M4.22 4.22l2.12 2.12M17.66 17.66l2.12 2.12M2 12h3M19 12h3M4.22 19.78l2.12-2.12M17.66 6.34l2.12-2.12" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>',
    fields: [
      {
        key: "api_base",
        label: l("API Base URL", "API 基础 URL"),
        type: "text",
        placeholder: "https://api.openai.com/v1",
        mono: true,
      },
      {
        key: "api_key",
        label: l("API Key", "API Key"),
        type: "password",
        placeholder: "sk-...",
      },
      {
        key: "model",
        label: l("Model", "模型"),
        type: "text",
        placeholder: "whisper-1",
        default: "whisper-1",
        mono: true,
      },
    ],
    hint: l(
      'Use any OpenAI-compatible endpoint. Examples: <code>whisper-1</code> (OpenAI), <code>groq/whisper-large-v3-turbo</code> (Groq via LiteLLM). See <a href="https://docs.litellm.ai/docs/providers" target="_blank" rel="noopener">LiteLLM docs</a>.',
      '支持任何 OpenAI 兼容端点。示例：<code>whisper-1</code>（OpenAI），<code>groq/whisper-large-v3-turbo</code>（通过 LiteLLM 使用 Groq）。详见 <a href="https://docs.litellm.ai/docs/providers" target="_blank" rel="noopener">LiteLLM 文档</a>。'
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
