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

export function getDefaultConfig(fields: ProviderField[]): Record<string, string> {
  const values: Record<string, string> = {};
  for (const field of fields) {
    if (field.default !== undefined) {
      values[field.key] = field.default;
    }
  }
  return values;
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
    id: "openai",
    name: "OpenAI",
    tagline: l("Hosted API", "官方 API"),
    icon: '<circle cx="12" cy="12" r="8" stroke="currentColor" stroke-width="1.8"/><path d="M8.5 12h7M12 8.5v7" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>',
    fields: [
      {
        key: "api_key",
        label: l("API Key", "API Key"),
        type: "password",
        placeholder: "sk-...",
      },
      {
        key: "model",
        label: l("Model", "模型"),
        type: "select",
        default: "gpt-4o-mini-transcribe",
        options: [
          { value: "gpt-4o-mini-transcribe", label: "GPT-4o mini Transcribe" },
          { value: "gpt-4o-transcribe", label: "GPT-4o Transcribe" },
          { value: "whisper-1", label: "Whisper" },
        ],
      },
    ],
    hint: l(
      'Uses <code>https://api.openai.com/v1</code>. Enter an OpenAI API key and pick a transcription model.',
      '使用 <code>https://api.openai.com/v1</code>。填写 OpenAI API Key 后选择转录模型即可。'
    ),
  },
  {
    id: "gemini",
    name: "Gemini",
    tagline: l("Google AI Studio", "Google AI Studio"),
    icon: '<path d="M12 2l2.2 6.1L20 10.3l-5.8 2.2L12 22l-2.2-9.5L4 10.3l5.8-2.2L12 2z" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"/>',
    fields: [
      {
        key: "api_key",
        label: l("API Key", "API Key"),
        type: "password",
        placeholder: "AIza...",
      },
      {
        key: "model",
        label: l("Model", "模型"),
        type: "select",
        default: "gemini-2.5-flash",
        options: [
          { value: "gemini-2.5-flash", label: "Gemini 2.5 Flash" },
          { value: "gemini-2.5-flash-lite", label: "Gemini 2.5 Flash-Lite" },
          { value: "gemini-2.5-pro", label: "Gemini 2.5 Pro" },
        ],
      },
    ],
    hint: l(
      'Uses the Gemini API directly. Create an API key in <a href="https://aistudio.google.com/app/apikey" target="_blank" rel="noopener">Google AI Studio</a>.',
      '直接使用 Gemini API。可在 <a href="https://aistudio.google.com/app/apikey" target="_blank" rel="noopener">Google AI Studio</a> 创建 API Key。'
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
        placeholder: "http://localhost:4000/v1",
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
      'For a self-hosted LiteLLM proxy or any custom OpenAI-compatible endpoint. Enter your own API base URL, API key, and model name.',
      '用于自建 LiteLLM Proxy 或其它自定义 OpenAI 兼容端点。请填写自己的 API Base URL、API Key 和模型名。'
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
