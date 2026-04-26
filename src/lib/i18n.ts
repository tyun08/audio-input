import { writable, derived } from "svelte/store";

export type Locale = "en" | "zh";

/** Bilingual string — used in providers.ts and other data-driven definitions. */
export type L = Record<Locale, string>;
export function l(en: string, zh: string): L {
  return { en, zh };
}

const stored =
  typeof localStorage !== "undefined"
    ? (localStorage.getItem("app-locale") as Locale | null)
    : null;

export const locale = writable<Locale>(stored || "en");

locale.subscribe((v) => {
  if (typeof localStorage !== "undefined") localStorage.setItem("app-locale", v);
});

const messages: Record<Locale, Record<string, string>> = {
  en: {
    // App / general
    "app.name": "Audio Input",
    "app.desc":
      "Press shortcut, speak, text auto-inputs to any app.\nmacOS & Windows · Groq Whisper & Google Vertex AI.",

    // Accessibility banner (App.svelte)
    "ax.need": "Accessibility permission is needed to auto-inject text",
    "ax.restart": "Fully quit and restart the app after granting",
    "ax.open": "Open System Settings",
    "ax.dismiss": "Dismiss",

    // Settings panel
    "settings.title": "Settings",
    "settings.recording": "Recording",
    "settings.transcribing": "Transcribing",
    "settings.voice_service": "Voice Service",
    "settings.save": "Save",
    "settings.saving": "Saving…",
    "settings.saved": "Saved",
    "settings.polish": "AI Polish",
    "settings.polish_desc": "Auto-punctuate and fix typos",
    "settings.mic": "Microphone",
    "settings.mic_default": "System Default",
    "settings.shortcut": "Global Shortcut",
    "settings.shortcut_apply": "Apply",
    "settings.shortcut_hint": "Meta = ⌘, Ctrl, Alt, Shift",
    "settings.shortcut_conflict":
      "Shortcut {0} may be occupied by another app. Try a different one.",
    "settings.autostart": "Launch at Login",
    "settings.autostart_desc": "Auto-start when you log in",
    "settings.screenshot": "Screenshot Context",
    "settings.screenshot_desc": "Capture screen while recording for better polish",
    "settings.show_idle_hud": "Show Idle Indicator",
    "settings.show_idle_hud_desc": "Keep mic icon visible when ready to record",
    "settings.language": "Language",

    // Settings nav tabs
    "settings.nav.transcription": "Transcription",
    "settings.nav.general": "General",
    "settings.nav.advanced": "Advanced",
    "settings.nav.history": "History",

    // Settings section headers
    "settings.section.startup": "Startup",
    "settings.section.input": "Input",
    "settings.section.language": "Language",

    // Onboarding
    "onboarding.start": "Get Started",
    "onboarding.configure": "Configure AI Service",
    "onboarding.save_continue": "Save & Continue",
    "onboarding.skip": "Skip",
    "onboarding.saved": "Saved",
    "onboarding.ax_title": "Grant Accessibility",
    "onboarding.ax_desc":
      "Audio Input needs Accessibility permission to type text into other apps.",
    "onboarding.ax_path1": "System Settings",
    "onboarding.ax_path2": "Privacy & Security",
    "onboarding.ax_path3": "Accessibility",
    "onboarding.ax_path4": "+ Audio Input",
    "onboarding.ax_done": "Done",
    "onboarding.ax_open": "Open System Settings",
    "onboarding.ready": "All Set!",
    "onboarding.ready_mac":
      "Press {0} to start recording. Release to auto-transcribe and type at cursor.\nClick the menu bar icon for settings.",
    "onboarding.ready_win":
      "Press {0} to start recording. Release to auto-transcribe and type at cursor.\nClick the system tray icon for settings.",
    "onboarding.finish": "Start Using",

    // HUD (RecordingIndicator)
    "hud.idle": "Ready",
    "hud.transcribing": "Transcribing…",
    "hud.error": "Error",
    "hud.copied": "Copied — ⌘V to paste",
    "hud.copied_title": "Copied to Clipboard",
    "hud.copied_detail": "No input focused — press ⌘V to paste",
    "hud.copy_again": "Copy Again",
    "history.copy": "Copy",
    "hud.polish_failed": "Polish failed — original used",
    "hud.retry_title": "Transcription failed",
    "hud.retry": "Retry",
    "hud.retrying": "Retrying…",
    "hud.dismiss": "Dismiss",
    "hud.success": "Sent ✓",
    "hud.success_detail": "Typed at cursor",

    // History tab
    "history.title": "History",
    "history.empty": "No recordings yet",
    "history.empty_hint": "Your recent recordings will appear here so you can retry or reuse them.",
    "history.status.completed": "Done",
    "history.status.failed": "Failed",
    "history.status.pending": "Processing…",
    "history.retry": "Retry",
    "history.delete": "Delete",
    "history.duration": "{0}s",
    "history.max_label": "Keep recent recordings",
    "history.max_desc": "Audio for the last N attempts is saved locally for retry.",
    "history.failed_unknown": "(Transcription failed — no error text)",
    "history.failed_hint":
      "Failed attempts are kept here with the error message so you can retry from Settings → History.",
  },

  zh: {
    "app.name": "Audio Input",
    "app.desc":
      "按下快捷键，说话，文字自动输入到任意应用。\nmacOS 与 Windows · Groq Whisper 和 Google Vertex AI。",

    "ax.need": "需要辅助功能权限才能自动注入文字",
    "ax.restart": "授权后请完全退出并重启 App",
    "ax.open": "打开系统设置",
    "ax.dismiss": "忽略",

    "settings.title": "设置",
    "settings.recording": "录音中",
    "settings.transcribing": "转录中",
    "settings.voice_service": "语音服务",
    "settings.save": "保存",
    "settings.saving": "保存中…",
    "settings.saved": "已保存",
    "settings.polish": "AI 润色",
    "settings.polish_desc": "自动添加标点、修正错字",
    "settings.mic": "麦克风",
    "settings.mic_default": "系统默认",
    "settings.shortcut": "全局快捷键",
    "settings.shortcut_apply": "应用",
    "settings.shortcut_hint": "Meta = ⌘，Ctrl，Alt，Shift",
    "settings.shortcut_conflict": "快捷键 {0} 可能已被其他应用占用，请尝试更换",
    "settings.autostart": "开机自启",
    "settings.autostart_desc": "登录时自动启动",
    "settings.screenshot": "截图上下文",
    "settings.screenshot_desc": "录音时截屏，提升润色准确度",
    "settings.show_idle_hud": "显示待机指示器",
    "settings.show_idle_hud_desc": "录音就绪时保持麦克风图标可见",
    "settings.language": "语言",

    // Settings nav tabs
    "settings.nav.transcription": "转录",
    "settings.nav.general": "通用",
    "settings.nav.advanced": "高级",
    "settings.nav.history": "历史",

    // Settings section headers
    "settings.section.startup": "启动",
    "settings.section.input": "输入",
    "settings.section.language": "语言",

    "onboarding.start": "开始配置",
    "onboarding.configure": "配置 AI 服务",
    "onboarding.save_continue": "保存并继续",
    "onboarding.skip": "跳过",
    "onboarding.saved": "已保存",
    "onboarding.ax_title": "授权辅助功能",
    "onboarding.ax_desc": "Audio Input 需要辅助功能权限才能将文字注入到其他应用。",
    "onboarding.ax_path1": "系统设置",
    "onboarding.ax_path2": "隐私与安全性",
    "onboarding.ax_path3": "辅助功能",
    "onboarding.ax_path4": "+ Audio Input",
    "onboarding.ax_done": "已完成",
    "onboarding.ax_open": "打开系统设置",
    "onboarding.ready": "准备就绪！",
    "onboarding.ready_mac":
      "按下 {0} 开始录音，松开自动转文字并输入到光标位置。\n点击菜单栏图标可打开设置。",
    "onboarding.ready_win":
      "按下 {0} 开始录音，松开自动转文字并输入到光标位置。\n点击系统托盘图标可打开设置。",
    "onboarding.finish": "开始使用",

    "hud.idle": "就绪",
    "hud.transcribing": "转录中…",
    "hud.error": "错误",
    "hud.copied": "已复制 — ⌘V 粘贴",
    "hud.copied_title": "已复制到剪贴板",
    "hud.copied_detail": "未检测到输入框 — 按 ⌘V 粘贴",
    "hud.copy_again": "重新复制",
    "history.copy": "复制",
    "hud.polish_failed": "润色失败 — 使用原文",
    "hud.retry_title": "转录失败",
    "hud.retry": "重试",
    "hud.retrying": "重试中…",
    "hud.dismiss": "忽略",
    "hud.success": "已写入 ✓",
    "hud.success_detail": "已键入到光标处",

    "history.title": "历史记录",
    "history.empty": "暂无录音",
    "history.empty_hint": "最近的录音会保存在这里，可随时重试或复用。",
    "history.status.completed": "完成",
    "history.status.failed": "失败",
    "history.status.pending": "处理中…",
    "history.retry": "重试",
    "history.delete": "删除",
    "history.duration": "{0} 秒",
    "history.max_label": "保留最近录音",
    "history.max_desc": "最近 N 次的音频会保存在本地以供重试。",
    "history.failed_unknown": "（转录失败 — 无错误详情）",
    "history.failed_hint":
      "失败记录会保留在此并显示错误信息，可在 设置 → 历史 中重试。",
  },
};

/**
 * Derived translation store. Use as `$t('key')` in Svelte templates.
 * Supports positional arguments: `$t('settings.shortcut_conflict', shortcut)`.
 */
export const t = derived(locale, ($locale) => {
  const dict = messages[$locale];
  const fallback = messages.en;
  return (key: string, ...args: string[]): string => {
    let msg = dict[key] ?? fallback[key] ?? key;
    args.forEach((arg, i) => {
      msg = msg.replace(`{${i}}`, arg);
    });
    return msg;
  };
});

/** Resolve a bilingual `L` object using the current locale (non-reactive, for imperative code). */
export function resolveL(ls: L, loc: Locale): string {
  return ls[loc] ?? ls.en;
}
