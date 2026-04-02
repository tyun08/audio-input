<script lang="ts">
  import { createEventDispatcher, onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  const dispatch = createEventDispatcher();

  let apiKey = "";
  let saving = false;
  let saved = false;
  let error = "";

  onMount(async () => {
    apiKey = await invoke<string>("get_saved_api_key");
  });

  async function handleSave() {
    if (!apiKey.trim()) {
      error = "API Key 不能为空";
      return;
    }
    saving = true;
    error = "";
    try {
      await invoke("save_api_key", { key: apiKey.trim() });
      saved = true;
      setTimeout(() => dispatch("saved"), 800);
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") handleSave();
    if (e.key === "Escape") dispatch("close");
  }
</script>

<div class="settings">
  <h2>配置 Groq API Key</h2>
  <p class="hint">
    在 <a href="https://console.groq.com" target="_blank" rel="noopener">console.groq.com</a>
    免费获取 API Key
  </p>

  <div class="field">
    <input
      type="password"
      placeholder="gsk_..."
      bind:value={apiKey}
      on:keydown={handleKeydown}
      autocomplete="off"
      spellcheck="false"
    />
  </div>

  {#if error}
    <div class="error">{error}</div>
  {/if}

  <div class="actions">
    <button class="cancel" on:click={() => dispatch("close")}>取消</button>
    <button class="save" on:click={handleSave} disabled={saving}>
      {#if saved}✓ 已保存{:else if saving}保存中...{:else}保存{/if}
    </button>
  </div>
</div>

<style>
  .settings {
    padding: 28px 24px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    width: 100%;
  }

  h2 {
    font-size: 16px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.9);
  }

  .hint {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.45);
    line-height: 1.5;
  }

  .hint a {
    color: #60a5fa;
    text-decoration: none;
  }

  .field input {
    width: 100%;
    padding: 10px 14px;
    border-radius: 10px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.9);
    font-size: 14px;
    font-family: "SF Mono", "Fira Code", monospace;
    outline: none;
    transition: border-color 0.15s;
  }

  .field input:focus {
    border-color: rgba(96, 165, 250, 0.5);
  }

  .field input::placeholder {
    color: rgba(255, 255, 255, 0.25);
  }

  .error {
    font-size: 12px;
    color: #f87171;
  }

  .actions {
    display: flex;
    gap: 10px;
    justify-content: flex-end;
    margin-top: 4px;
  }

  button {
    padding: 8px 20px;
    border-radius: 8px;
    border: none;
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s;
  }

  .cancel {
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.6);
  }

  .cancel:hover {
    background: rgba(255, 255, 255, 0.12);
  }

  .save {
    background: #2563eb;
    color: white;
  }

  .save:hover:not(:disabled) {
    background: #1d4ed8;
  }

  .save:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
</style>
