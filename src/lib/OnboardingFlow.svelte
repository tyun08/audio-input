<script lang="ts">
  import { createEventDispatcher, onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { providers, getProvider, groupFields } from "./providers";

  const dispatch = createEventDispatcher();

  const isWindows = navigator.userAgent.includes("Windows");
  const totalSteps = isWindows ? 3 : 4;

  let step = 1;
  let provider = providers[0]?.id ?? "groq";
  let configValues: Record<string, string> = {};
  let authStatus: boolean | null = null;

  let configSaving = false;
  let configSaved = false;
  let configError = "";

  $: currentProvider = getProvider(provider);
  $: fieldGroups = groupFields(currentProvider?.fields ?? []);

  onMount(async () => {
    for (const p of providers) {
      if (p.authCheck) {
        const ok = await invoke<boolean>(p.authCheck, { provider: p.id }).catch(() => false);
        if (p.id === provider) authStatus = ok;
      }
    }
  });

  async function handleProviderSwitch(id: string) {
    provider = id;
    configValues = {};
    configError = "";
    const cp = getProvider(id);
    if (cp?.authCheck) {
      authStatus = await invoke<boolean>(cp.authCheck, { provider: id }).catch(() => false);
    } else {
      authStatus = null;
    }
  }

  async function saveConfig() {
    configSaving = true;
    configError = "";
    try {
      await invoke("save_provider", { provider });
      await invoke("save_provider_config", { provider, configValues });
      configSaved = true;
      setTimeout(() => { step = isWindows ? totalSteps : 3; }, 500);
    } catch (e) {
      configError = String(e);
    } finally {
      configSaving = false;
    }
  }

  async function skipConfig() {
    await invoke("save_provider", { provider });
    step = isWindows ? totalSteps : 3;
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
      <p class="app-desc">按下快捷键，说话，文字自动输入到任意应用。</p>
      <button class="primary-btn" on:click={next}>开始配置</button>
    </div>

  <!-- Step 2: Provider + Config -->
  {:else if step === 2}
    <div class="step">
      <div class="step-num">{step} / {totalSteps}</div>
      <h2>配置 AI 服务</h2>

      <div class="provider-tabs">
        {#each providers as p}
          <button
            class="provider-tab"
            class:active={provider === p.id}
            on:click={() => handleProviderSwitch(p.id)}
          >
            <svg width="15" height="15" viewBox="0 0 24 24" fill="none">{@html p.icon}</svg>
            <div class="tab-text">
              <span class="tab-name">{p.name}</span>
              <span class="tab-desc">{p.tagline}</span>
            </div>
          </button>
        {/each}
      </div>

      {#if currentProvider?.hint}
        <p class="desc">{@html currentProvider.hint}</p>
      {/if}

      <!-- Dynamic config fields -->
      {#each fieldGroups as group}
        {#if group.length === 1}
          {#if group[0].type === "select"}
            <select class="config-select" bind:value={configValues[group[0].key]}>
              {#each group[0].options ?? [] as opt}
                <option value={opt.value}>{opt.label}</option>
              {/each}
            </select>
          {:else if group[0].type === "password"}
            <input
              type="password"
              class="config-input"
              placeholder={group[0].placeholder ?? group[0].label}
              bind:value={configValues[group[0].key]}
              on:keydown={(e) => e.key === "Enter" && saveConfig()}
              autocomplete="off"
              spellcheck="false"
            />
          {:else}
            <input
              type="text"
              class="config-input"
              class:mono={group[0].mono}
              placeholder={group[0].placeholder ?? group[0].label}
              bind:value={configValues[group[0].key]}
              on:keydown={(e) => e.key === "Enter" && saveConfig()}
              autocomplete="off"
              spellcheck="false"
            />
          {/if}
        {:else}
          <div class="mini-row">
            {#each group as field}
              {#if field.type === "select"}
                <select class="config-select" bind:value={configValues[field.key]}>
                  {#each field.options ?? [] as opt}
                    <option value={opt.value}>{opt.label}</option>
                  {/each}
                </select>
              {:else}
                <input
                  type="text"
                  class="config-input mini"
                  class:mono={field.mono}
                  placeholder={field.placeholder ?? field.label}
                  bind:value={configValues[field.key]}
                  autocomplete="off"
                  spellcheck="false"
                />
              {/if}
            {/each}
          </div>
        {/if}
      {/each}

      {#if authStatus !== null && currentProvider?.authOkText}
        <div class="auth-badge" class:ok={authStatus}>
          <div class="auth-dot"></div>
          <span>{authStatus ? currentProvider.authOkText : currentProvider.authFailText}</span>
        </div>
      {/if}

      {#if configError}
        <div class="err">{configError}</div>
      {/if}
      <div class="btn-row">
        <button class="ghost-btn" on:click={skipConfig}>跳过</button>
        <button class="primary-btn" on:click={saveConfig} disabled={configSaving}>
          {#if configSaved}已保存{:else if configSaving}保存中...{:else}保存并继续{/if}
        </button>
      </div>
    </div>

  <!-- Step 3: Accessibility (macOS) -->
  {:else if step === 3 && !isWindows}
    <div class="step">
      <div class="step-num">{step} / {totalSteps}</div>
      <h2>授权辅助功能</h2>
      <p class="desc">Audio Input 需要辅助功能权限才能将文字注入到其他应用。</p>
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
        <div class="path-step"><span>隐私与安全性</span></div>
        <div class="path-arrow">›</div>
        <div class="path-step"><span>辅助功能</span></div>
        <div class="path-arrow">›</div>
        <div class="path-step highlight"><span>+ Audio Input</span></div>
      </div>
      <div class="btn-row">
        <button class="ghost-btn" on:click={next}>已完成</button>
        <button class="primary-btn" on:click={() => invoke("open_accessibility_prefs")}>打开系统设置</button>
      </div>
    </div>

  <!-- Final step: Done -->
  {:else if step === totalSteps}
    <div class="step">
      <div class="done-check">
        <svg width="28" height="28" viewBox="0 0 28 28" fill="none">
          <circle cx="14" cy="14" r="13" stroke="rgba(74,222,128,0.7)" stroke-width="2"/>
          <path d="M8 14l4 4 8-8" stroke="rgba(74,222,128,0.9)" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
      </div>
      <h2>准备就绪！</h2>
      <p class="desc">
        按下 <kbd>{isWindows ? "Ctrl+Shift+Space" : "⌘⇧Space"}</kbd> 开始录音，松开自动转文字并输入到光标位置。
        <br>点击{isWindows ? "系统托盘" : "菜单栏"}图标可打开设置。
      </p>
      <button class="primary-btn" on:click={finishOnboarding}>开始使用</button>
    </div>
  {/if}
</div>

<style>
  .onboarding { width:320px; background:rgba(30,30,32,0.92); backdrop-filter:blur(20px) saturate(180%); -webkit-backdrop-filter:blur(20px) saturate(180%); border:1px solid rgba(255,255,255,0.12); border-radius:20px; box-shadow:0 8px 40px rgba(0,0,0,0.5); padding:24px 24px 20px; display:flex; flex-direction:column; gap:16px; font-family:-apple-system,"SF Pro Text",BlinkMacSystemFont,sans-serif; -webkit-font-smoothing:antialiased; }
  .dots { display:flex; gap:6px; justify-content:center; }
  .dot { width:6px; height:6px; border-radius:50%; background:rgba(255,255,255,0.15); transition:background .2s,transform .2s; }
  .dot.active { background:rgba(129,140,248,0.9); transform:scale(1.3); }
  .dot.done { background:rgba(129,140,248,0.4); }
  .step { display:flex; flex-direction:column; align-items:center; gap:12px; text-align:center; }
  .step-num { font-size:11px; color:rgba(255,255,255,0.3); letter-spacing:.05em; }
  .app-icon { margin-top:4px; }
  .app-name { font-size:22px; font-weight:700; color:rgba(255,255,255,0.92); letter-spacing:-0.02em; }
  h2 { font-size:17px; font-weight:600; color:rgba(255,255,255,0.9); letter-spacing:-0.01em; }
  .app-desc,.desc { font-size:13px; color:rgba(255,255,255,0.5); line-height:1.6; max-width:260px; }
  .desc :global(a) { color:rgba(129,140,248,0.85); text-decoration:none; }
  .desc :global(code) { font-family:"SF Mono","Fira Code",monospace; font-size:10px; background:rgba(255,255,255,0.06); padding:1px 4px; border-radius:3px; color:rgba(255,255,255,0.5); }

  /* Provider tabs */
  .provider-tabs { display:flex; gap:8px; width:100%; flex-wrap:wrap; }
  .provider-tab { flex:1; min-width:0; display:flex; align-items:center; gap:8px; padding:10px 12px; border-radius:12px; border:1.5px solid rgba(255,255,255,0.08); background:rgba(255,255,255,0.03); color:rgba(255,255,255,0.4); cursor:pointer; transition:all .2s ease; font-family:-apple-system,"SF Pro Text",BlinkMacSystemFont,sans-serif; text-align:left; }
  .provider-tab:hover { border-color:rgba(255,255,255,0.15); color:rgba(255,255,255,0.6); }
  .provider-tab.active { border-color:rgba(129,140,248,0.45); background:rgba(99,102,241,0.08); color:rgba(165,180,252,0.95); }
  .tab-text { display:flex; flex-direction:column; gap:1px; }
  .tab-name { font-size:13px; font-weight:600; }
  .tab-desc { font-size:10px; opacity:0.55; font-weight:400; }

  /* Config inputs */
  .config-input { width:100%; padding:9px 12px; border-radius:10px; border:1px solid rgba(255,255,255,0.1); background:rgba(255,255,255,0.05); color:rgba(255,255,255,0.9); font-size:13px; font-family:-apple-system,"SF Pro Text",BlinkMacSystemFont,sans-serif; outline:none; text-align:left; transition:border-color .15s; }
  .config-input.mono { font-family:"SF Mono","Fira Code",monospace; font-size:12px; }
  .config-input:focus { border-color:rgba(129,140,248,0.5); }
  .config-input::placeholder { color:rgba(255,255,255,0.2); }
  .mini-row { display:flex; gap:8px; width:100%; }
  .config-input.mini { flex:1; min-width:0; }
  .config-select { flex:1; min-width:0; padding:9px 8px; border-radius:10px; border:1px solid rgba(255,255,255,0.1); background:rgba(255,255,255,0.05); color:rgba(255,255,255,0.9); font-size:12px; font-family:-apple-system,"SF Pro Text",BlinkMacSystemFont,sans-serif; outline:none; cursor:pointer; appearance:auto; }
  .err { font-size:12px; color:#f87171; align-self:flex-start; }

  /* Auth badge */
  .auth-badge { display:flex; align-items:center; gap:6px; font-size:11px; color:rgba(248,113,113,0.75); width:100%; padding:6px 10px; border-radius:8px; background:rgba(248,113,113,0.05); border:1px solid rgba(248,113,113,0.1); text-align:left; }
  .auth-badge.ok { color:rgba(134,239,172,0.8); background:rgba(74,222,128,0.05); border-color:rgba(74,222,128,0.12); }
  .auth-dot { width:5px; height:5px; border-radius:50%; background:rgba(248,113,113,0.65); flex-shrink:0; }
  .auth-badge.ok .auth-dot { background:rgba(74,222,128,0.65); }

  /* Accessibility */
  .ax-illustration { display:flex; align-items:center; gap:4px; background:rgba(255,255,255,0.04); border:1px solid rgba(255,255,255,0.08); border-radius:10px; padding:10px 12px; flex-wrap:wrap; justify-content:center; width:100%; }
  .path-step { display:flex; align-items:center; gap:5px; font-size:11px; color:rgba(255,255,255,0.55); }
  .path-step.highlight { color:rgba(74,222,128,0.85); font-weight:600; }
  .path-arrow { font-size:13px; color:rgba(255,255,255,0.2); }
  .path-icon { display:flex; align-items:center; }
  .done-check { margin-top:8px; }
  kbd { display:inline-block; padding:2px 6px; border-radius:5px; border:1px solid rgba(255,255,255,0.15); background:rgba(255,255,255,0.06); font-family:-apple-system,"SF Pro Text",BlinkMacSystemFont,sans-serif; font-size:12px; color:rgba(255,255,255,0.75); }

  /* Buttons */
  .btn-row { display:flex; gap:10px; width:100%; justify-content:center; margin-top:4px; }
  .primary-btn { padding:9px 22px; border-radius:10px; border:none; background:rgba(99,102,241,0.85); color:white; font-size:13px; font-weight:600; cursor:pointer; transition:background .15s; font-family:-apple-system,"SF Pro Text",BlinkMacSystemFont,sans-serif; }
  .primary-btn:hover:not(:disabled) { background:rgba(99,102,241,1); }
  .primary-btn:disabled { opacity:0.55; cursor:not-allowed; }
  .ghost-btn { padding:9px 18px; border-radius:10px; border:1px solid rgba(255,255,255,0.12); background:rgba(255,255,255,0.06); color:rgba(255,255,255,0.55); font-size:13px; font-weight:500; cursor:pointer; transition:background .15s; font-family:-apple-system,"SF Pro Text",BlinkMacSystemFont,sans-serif; }
  .ghost-btn:hover { background:rgba(255,255,255,0.1); }
</style>
