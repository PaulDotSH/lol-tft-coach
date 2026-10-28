<script lang="ts">
  import type { AppState } from '../lib/bindings/AppState';
  import type { SourceState } from '../lib/bindings/SourceState';
  import { clock, saveSettings } from '../lib/store.svelte';
  import { clockText, MODE_NAMES } from '../lib/ui';

  let { state }: { state: AppState } = $props();

  const PHASES = {
    noclient: 'No client',
    idle: 'Between games',
    champselect: 'Champion select',
    loading: 'Loading into game',
    ingame: 'In game',
    postgame: 'Game over',
  } as const;

  const mode = $derived(state.game?.mode ?? state.champ_select?.mode);
  const dots = $derived<[string, SourceState][]>([
    ['Client', state.sources.client],
    ['Game', state.sources.game],
    ['lolalytics', state.sources.lolalytics],
    ['u.gg', state.sources.ugg],
    ['Coachless', state.sources.coachless],
    ['OCR', state.sources.ocr],
  ]);
  const tip = $derived(state.sources.message ?? '');
</script>

<header>
  <span class="brand">LoL Coach</span>
  <span class="phase">{PHASES[state.phase]}</span>
  {#if mode}<span class="muted">{MODE_NAMES[mode]}</span>{/if}
  {#if state.phase === 'ingame' && state.game}
    <span class="clock num">{clockText(clock.time)}</span>
  {/if}
  {#if state.phase === 'ingame'}
    <!-- In game the settings panel is hidden: the overlay toggle stays reachable here. -->
    <label class="overlay" title="Over the game window; borderless or windowed (exclusive fullscreen hides it)">
      <input
        type="checkbox"
        checked={state.settings.overlay}
        onchange={(e) => saveSettings({ ...state.settings, overlay: e.currentTarget.checked })}
      />
      Overlay
    </label>
  {/if}
  <ul class="sources" title={tip} aria-label="Data sources">
    {#each dots as [name, st] (name)}
      <li class={st} title={tip ? `${name}: ${st}. ${tip}` : `${name}: ${st}`}>{name}</li>
    {/each}
  </ul>
</header>

<style>
  header {
    display: flex;
    align-items: baseline;
    gap: 14px;
    padding: 8px var(--gap);
    border-bottom: 1px solid var(--line);
    background: var(--panel);
  }
  .brand {
    font: 700 17px var(--cond);
    color: var(--gold);
  }
  .phase {
    font: 600 17px var(--cond);
  }
  .clock {
    font-size: 17px;
    color: var(--muted);
  }
  .overlay {
    margin-left: auto;
    font-size: 13px;
    color: var(--muted);
  }
  .overlay + .sources {
    margin-left: 0;
  }
  .sources {
    margin: 0 0 0 auto;
    padding: 0;
    list-style: none;
    display: flex;
    gap: 12px;
    font-size: 13px;
    color: var(--muted);
    cursor: default;
  }
  li::before {
    content: '';
    display: inline-block;
    width: 8px;
    height: 8px;
    margin-right: 5px;
    border-radius: 50%;
    background: var(--faint);
    vertical-align: 1px;
  }
  .ok::before {
    background: var(--good);
  }
  .degraded::before {
    background: var(--warn);
  }
  .error::before {
    background: var(--threat);
  }
  .off {
    color: var(--faint);
  }
  .off::before {
    background: none;
    box-shadow: inset 0 0 0 1.5px var(--faint);
  }
</style>
