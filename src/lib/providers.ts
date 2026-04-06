/**
 * Provider registry — the single source of truth for all LLM providers.
 *
 * To add a new provider:
 *   1. Add an entry to `providers` below (frontend metadata + fields).
 *   2. Create `src-tauri/src/transcription/<id>.rs` (transcribe + polish).
 *   3. Add a match arm in `commands.rs` → `transcribe_with_provider` / `polish_with_provider`.
 *   That's it — no new IPC commands, no config schema changes.
 */

export interface ProviderField {
  key: string;
  label: string;
  type: "text" | "password" | "select";
  placeholder?: string;
  options?: { value: string; label: string }[];
  mono?: boolean;
  /** If true, this field and the next `half` field share one row. */
  half?: boolean;
}

export interface ProviderDef {
  id: string;
  name: string;
  tagline: string;
  icon: string; // SVG inner content for a 24×24 viewBox
  fields: ProviderField[];
  /** IPC command that returns boolean — called with `{ provider: id }` */
  authCheck?: string;
  authOkText?: string;
  authFailText?: string;
  /** HTML string shown below the config form */
  hint?: string;
}

export const providers: ProviderDef[] = [
  {
    id: "groq",
    name: "Groq",
    tagline: "免费 API Key",
    icon: '<path d="M13 2L3 14h9l-1 8 10-12h-9l1-8z" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>',
    fields: [
      {
        key: "api_key",
        label: "API Key",
        type: "password",
        placeholder: "gsk_...",
      },
    ],
    hint: '在 <a href="https://console.groq.com" target="_blank" rel="noopener">console.groq.com</a> 免费获取',
  },
  {
    id: "vertex_ai",
    name: "Vertex AI",
    tagline: "Google Cloud",
    icon: '<path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"/><polyline points="3.27 6.96 12 12.01 20.73 6.96" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/><line x1="12" y1="22.08" x2="12" y2="12" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>',
    fields: [
      {
        key: "project_id",
        label: "GCP 项目 ID",
        type: "text",
        placeholder: "my-project-id",
      },
      {
        key: "location",
        label: "区域",
        type: "text",
        placeholder: "us-central1",
        mono: true,
        half: true,
      },
      {
        key: "model",
        label: "模型",
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
    authOkText: "gcloud 凭证已就绪",
    authFailText: "未检测到 gcloud 凭证",
    hint: '请运行 <code>gcloud auth application-default login</code>',
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
