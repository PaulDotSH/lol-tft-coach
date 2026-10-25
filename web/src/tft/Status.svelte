<script lang="ts">
  // One small line of TFT data sources (set data, meta, screen reading) with ok / down dots,
  // and the shared "speak calls" toggle, since the LoL settings panel is hidden in TFT.
  import { saveSettings, store } from '../lib/store.svelte';

  let { status }: { status: [string, boolean][] } = $props();

  const settings = $derived(store.state?.settings ?? null);
</script>

<footer>
  <ul aria-label="TFT data sources">
    {#each status as [name, ok] (name)}
      <li class:ok title="{name}: {ok ? 'ok' : 'not available'}">{name}</li>
    {/each}
  </ul>
  {#if settings}
    <label>
      <input
        type="checkbox"
        checked={settings.audio}
        onchange={(e) => saveSettings({ ...settings, audio: e.currentTarget.checked })}
      />
      Speak calls
    </label>
  {/if}
</footer>

<style>
  footer {
    display: flex;
    align-items: center;
    gap: 16px;
    font-size: 13px;
    color: var(--muted);
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    gap: 12px;
  }
  li::before {
    content: '';
    display: inline-block;
    width: 8px;
    height: 8px;
    margin-right: 5px;
    border-radius: 50%;
    background: var(--threat);
    vertical-align: 1px;
  }
  li.ok::before {
    background: var(--good);
  }
  label {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
  }
</style>
