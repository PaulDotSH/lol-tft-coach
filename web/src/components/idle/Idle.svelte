<script lang="ts">
  import type { AppState } from '../../lib/bindings/AppState';
  import Recap from './Recap.svelte';
  import Lookup from './Lookup.svelte';
  import SettingsPanel from './SettingsPanel.svelte';

  let { state }: { state: AppState } = $props();

  const STATUS: Partial<Record<AppState['phase'], string>> = {
    noclient: 'Waiting for League client…',
    loading: 'Loading into game…',
    idle: 'Connected to the League client.',
    postgame: 'Game over. Recap below.',
  };
</script>

<div class="idle">
  <div class="left">
    <p class="status" class:waiting={state.phase === 'noclient' || state.phase === 'loading'}>
      {STATUS[state.phase] ?? ''}
    </p>
    {#if state.recap}
      <Recap recap={state.recap} />
    {:else}
      <section class="panel"><h2>Last game</h2><p class="empty">Play a game to see a recap here.</p></section>
    {/if}
    <SettingsPanel settings={state.settings} />
  </div>
  <Lookup />
</div>

<style>
  .idle {
    display: grid;
    grid-template-columns: minmax(320px, 2fr) minmax(0, 3fr);
    gap: var(--gap);
    align-items: start;
  }
  .left {
    display: grid;
    gap: var(--gap);
  }
  .status {
    margin: 0;
    font: 600 20px var(--cond);
  }
  .waiting {
    color: var(--gold);
    animation: breathe 2.4s ease-in-out infinite;
  }
  @keyframes breathe {
    50% {
      opacity: 0.55;
    }
  }
  @media (max-width: 760px) {
    .idle {
      grid-template-columns: 1fr;
    }
  }
</style>
