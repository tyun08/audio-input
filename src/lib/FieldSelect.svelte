<script lang="ts">
  /** Reusable option selector.
   *  ≤ 2 options → segmented/toggle control
   *  ≥ 3 options → native <select> dropdown
   */
  export let options: { value: string; label: string }[] = [];
  export let value: string = "";
</script>

{#if options.length === 2}
  <div class="seg">
    {#each options as opt}
      <button
        class:active={value === opt.value}
        on:click={() => (value = opt.value)}
        type="button"
      >{opt.label}</button>
    {/each}
  </div>
{:else}
  <select class="dropdown" bind:value>
    {#each options as opt}
      <option value={opt.value}>{opt.label}</option>
    {/each}
  </select>
{/if}

<style>
  /* ── Segmented control ── */
  .seg {
    display: flex;
    background: rgba(0, 0, 0, 0.07);
    border-radius: 8px;
    padding: 2px;
  }
  .seg button {
    padding: 4px 14px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: #3c3c3e;
    font-size: 13px;
    font-weight: 400;
    cursor: pointer;
    transition: all 0.15s;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
    white-space: nowrap;
  }
  .seg button.active {
    background: white;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.18);
    color: #1c1c1e;
    font-weight: 500;
  }

  /* ── Dropdown ── */
  .dropdown {
    font-size: 13px;
    color: #1c1c1e;
    background: #f2f2f2;
    border: 1px solid rgba(0, 0, 0, 0.12);
    border-radius: 7px;
    padding: 5px 7px;
    outline: none;
    cursor: pointer;
    max-width: 200px;
    font-family: -apple-system, "SF Pro Text", BlinkMacSystemFont, sans-serif;
  }
</style>
