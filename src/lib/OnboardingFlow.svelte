<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  const dispatch = createEventDispatcher();

  let step = 1;
  const totalSteps = 4;

  let apiKey = "";
  let apiKeySaving = false;
  let apiKeySaved = false;
  let apiKeyError = "";

  async function saveApiKey() {
    if (!apiKey.trim()) {
      apiKeyError = "请输入 API Key";
      return;
    }
    apiKeySaving = true;
    apiKeyError = "";
    try {
      await invoke("save_api_key", { key: apiKey.trim() });
      apiKeySaved = true;
      setTimeout(() => { step = 3; }, 600);
    } catch (e) {
      apiKeyError = String(e);
    } finally {
      apiKeySaving = false;
    }
  }

  async function finishOnboarding() {
    await invoke("save_onboarding_completed");
    dispatch("done");
  }

  function next() {
    if (step < totalSteps) step++;
  }
</script>

<div class="onboarding">
  <!-- Progress dots -->
  <div class="dots">
    {#each Array(totalSteps) as _, i}
      <div class="dot" class:active={i + 1 === step} class:done={i + 1 < step}></div>
    {/each}
  </div>

  <!-- Step 1: Welcome -->
  {#if step === 1}
    <div class="step">
      <div class="app-icon">
        <svg width="44" height="44" viewBox="0 0 48 48" fill="none">
          <rect width="48" height="48" rx="12" fill="rgba(99,102,241,0.18)"/>
          <rect x="19" y="8" width="10" height="22" rx="5" fill="rgba(129,140,248,0.9)"/>
          <path d="M10 22a14 14 0 0 0 28 0" stroke="rgba(129,140,248,0.9)" stroke-width="3" stroke-linecap="round"/>
          <line x1="24" y1="36" x2="24" y2="42" stroke="rgba(129,140,248,0.9)" stroke-width="3" stroke-linecap="round"/>
          <line x1="18" y1="42" x2="30" y2="42" stroke="rgba(129,140,248,0.9)" stroke-width="3" stroke-linecap="round"/>
        </svg>
      </div>
      <h1 class="app-name">Audio Input</h1>
      <p class="app-desc">按下快捷键，说话，文字自动输入到任意应用。<br>基于 Groq Whisper 的极速语音转文字。</p>
      <button class="primary-btn" on:click={next}>开始配置</button>
    </div>

  <!-- Step 2: API Key -->
  {:else if step === 2}
    <div class="step">
      <div class="step-num">2 / 4</div>
      <h2>配置 Groq API Key</h2>
      <p class="desc">
        前往 <a href="https://console.groq.com" target="_blank" rel="noopener">console.groq.com</a> 免费注册并获取 API Key（免费额度足够日常使用）。
      </p>
      <input
        type="password"
        class="api-input"
        placeholder="gsk_..."
        bind:value={apiKey}
        on:keydown={(e) => e.key === "Enter" && saveApiKey()}
        autocomplete="off"
        spellcheck="false"
      />
      {#if apiKeyError}
        <div class="err">{apiKeyError}</div>
      {/if}
      <div class="btn-row">
        <button class="ghost-btn" on:click={() => { step = 3; }}>跳过</button>
        <button class="primary-btn" on:click={saveApiKey} disabled={apiKeySaving}>
          {#if apiKeySaved}已保存{:else if apiKeySaving}保存中...{:else}保存并继续{/if}
        </button>
      </div>
    </div>

  <!-- Step 3: Accessibility -->
  {:else if step === 3}
    <div class="step">
      <div class="step-num">3 / 4</div>
      <h2>授权辅助功能</h2>
      <p class="desc">
        Audio Input 需要辅助功能权限才能将文字注入到其他应用。这是必要权限，不用于任何其他用途。
      </p>

      <!-- CSS illustration of system settings path -->
      <div class="ax-illustration">
        <div class="path-step">
          <div class="path-icon">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none">
              <circle cx="12" cy="12" r="9" stroke="rgba(129,140,248,0.8)" stroke-width="1.8"/>
              <circle cx="12" cy="8" r="1.5" fill="rgba(129,140,248,0.8)"/>
              <line x1="12" y1="12" x2="12" y2="18" stroke="rgba(129,140,248,0.8)" stroke-width="1.8" stroke-linecap="round"/>
            </svg>
          </div>
          <span>系统设置</span>
        </div>
        <div class="path-arrow">›</div>
        <div class="path-step">
          <span>隐私与安全性</span>
        </div>
        <div class="path-arrow">›</div>
        <div class="path-step">
          <span>辅助功能</span>
        </div>
        <div class="path-arrow">›</div>
        <div class="path-step highlight">
          <span>+ Audio Input</span>
        </div>
      </div>

      <div class="btn-row">
        <button class="ghost-btn" on:click={next}>已完成</button>
        <button class="primary-btn" on:click={() => invoke("open_accessibility_prefs")}>打开系统设置</button>
      </div>
    </div>

  <!-- Step 4: Done -->
  {:else if step === 4}
    <div class="step">
      <div class="done-check">
        <svg width="28" height="28" viewBox="0 0 28 28" fill="none">
          <circle cx="14" cy="14" r="13" stroke="rgba(74,222,128,0.7)" stroke-width="2"/>
          <path d="M8 14l4 4 8-8" stroke="rgba(74,222,128,0.9)" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
      </div>
      <h2>准备就绪！</h2>
      <p class="desc">
        按下 <kbd>⌘⇧Space</kbd> 开始录音，松开自动转文字并输入到光标位置。
        <br>点击菜单栏图标可打开设置。
      </p>
      <button class="primary-btn" on:click={finishOnboarding}>开始使用</button>
    </div>
  {/if}
</div>

<style>
  .onboarding {
    width: 320px;
    background: rgba(30, 30, 32, 0.92);
    backdrop-filter: blur(20px) saturate(180%);
    -webkit-backdrop-filter: blur(20px) saturate(180%);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 20px;
    box-shadow: 0 8px 40px rgba(0, 0, 0, 0.5);
    padding: 24px 24px 20px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    -webkit-font-smoothing: antialiased;
  }

  /* Progress dots */
  .dots {
    display: flex;
    gap: 6px;
    justify-content: center;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: rgba(255,255,255,0.15);
    transition: background 0.2s, transform 0.2s;
  }

  .dot.active {
    background: rgba(129, 140, 248, 0.9);
    transform: scale(1.3);
  }

  .dot.done {
    background: rgba(129, 140, 248, 0.4);
  }

  /* Step container */
  .step {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    text-align: center;
  }

  .step-num {
    font-size: 11px;
    color: rgba(255,255,255,0.3);
    letter-spacing: 0.05em;
  }

  .app-icon {
    margin-top: 4px;
  }

  .app-name {
    font-size: 22px;
    font-weight: 700;
    color: rgba(255,255,255,0.92);
    letter-spacing: -0.02em;
  }

  h2 {
    font-size: 17px;
    font-weight: 600;
    color: rgba(255,255,255,0.9);
    letter-spacing: -0.01em;
  }

  .app-desc, .desc {
    font-size: 13px;
    color: rgba(255,255,255,0.5);
    line-height: 1.6;
    max-width: 260px;
  }

  .desc a {
    color: rgba(129, 140, 248, 0.85);
    text-decoration: none;
  }

  .api-input {
    width: 100%;
    padding: 9px 12px;
    border-radius: 10px;
    border: 1px solid rgba(255,255,255,0.1);
    background: rgba(255,255,255,0.05);
    color: rgba(255,255,255,0.9);
    font-size: 13px;
    font-family: "SF Mono", "Fira Code", monospace;
    outline: none;
    text-align: left;
    transition: border-color 0.15s;
  }

  .api-input:focus {
    border-color: rgba(129, 140, 248, 0.5);
  }

  .api-input::placeholder {
    color: rgba(255,255,255,0.2);
  }

  .err {
    font-size: 12px;
    color: #f87171;
    align-self: flex-start;
  }

  /* Accessibility illustration */
  .ax-illustration {
    display: flex;
    align-items: center;
    gap: 4px;
    background: rgba(255,255,255,0.04);
    border: 1px solid rgba(255,255,255,0.08);
    border-radius: 10px;
    padding: 10px 12px;
    flex-wrap: wrap;
    justify-content: center;
    width: 100%;
  }

  .path-step {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 11px;
    color: rgba(255,255,255,0.55);
  }

  .path-step.highlight {
    color: rgba(74, 222, 128, 0.85);
    font-weight: 600;
  }

  .path-arrow {
    font-size: 13px;
    color: rgba(255,255,255,0.2);
  }

  .path-icon {
    display: flex;
    align-items: center;
  }

  /* Done state */
  .done-check {
    margin-top: 8px;
  }

  kbd {
    display: inline-block;
    padding: 2px 6px;
    border-radius: 5px;
    border: 1px solid rgba(255,255,255,0.15);
    background: rgba(255,255,255,0.06);
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    font-size: 12px;
    color: rgba(255,255,255,0.75);
  }

  /* Buttons */
  .btn-row {
    display: flex;
    gap: 10px;
    width: 100%;
    justify-content: center;
    margin-top: 4px;
  }

  .primary-btn {
    padding: 9px 22px;
    border-radius: 10px;
    border: none;
    background: rgba(99, 102, 241, 0.85);
    color: white;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
  }

  .primary-btn:hover:not(:disabled) {
    background: rgba(99, 102, 241, 1);
  }

  .primary-btn:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .ghost-btn {
    padding: 9px 18px;
    border-radius: 10px;
    border: 1px solid rgba(255,255,255,0.12);
    background: rgba(255,255,255,0.06);
    color: rgba(255,255,255,0.55);
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
  }

  .ghost-btn:hover {
    background: rgba(255,255,255,0.1);
  }
</style>
