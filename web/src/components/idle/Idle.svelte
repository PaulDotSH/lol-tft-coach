<script lang="ts">
  import type { AppState } from '../../lib/bindings/AppState';
  import Analysis from './Analysis.svelte';
  import DeathMapPanel from './DeathMapPanel.svelte';
  import History from './History.svelte';
  import Recap from './Recap.svelte';
  import Lookup from './Lookup.svelte';
  import MacroPatterns from './MacroPatterns.svelte';
  import SettingsPanel from './SettingsPanel.svelte';
  import Session from './Session.svelte';
  import Rank from './Rank.svelte';
  import Trends from './Trends.svelte';

  let { state: app }: { state: AppState } = $props();

  const STATUS: Partial<Record<AppState['phase'], string>> = {
    noclient: 'Waiting for League client…',
    loading: 'Loading into game…',
    idle: 'Connected to the League client.',
    postgame: 'Game over. Recap below.',
  };

  /** Game under review, from the match history. */
  let open = $state<number | null>(null);
  const connected = $derived(app.sources.client === 'ok');
  // The recap's game, once it reaches the history list (same champion and length).
  const recapGame = $derived.by(() => {
    const s = app.recap?.stats;
    const g = app.history[0];
    return s && g && g.champ === s.champ && Math.abs(g.duration_s - s.duration_s) <= 2 ? g.game_id : null;
  });
</script>

{#if open != null}
  <Analysis gameId={open} onclose={() => (open = null)} />
{:else}
  <div class="idle">
    <div class="left">
      <p class="status" class:waiting={app.phase === 'noclient' || app.phase === 'loading'}>
        {STATUS[app.phase] ?? ''}
      </p>
      {#if app.recap}
        <Recap recap={app.recap} onreview={recapGame != null && connected ? () => (open = recapGame) : undefined} />
      {:else}
        <section class="panel"><h2>Last game</h2><p class="empty">Play a game to see a recap here.</p></section>
      {/if}
      <SettingsPanel settings={app.settings} sources={app.sources} readers={app.readers} sync={app.sync} />
    </div>
    <div class="right">
      {#if app.trends?.session}<Session session={app.trends.session} games={app.history} />{/if}
      {#if app.ranked.length}<Rank ranked={app.ranked} />{/if}
      <History games={app.history} {connected} onopen={(id) => (open = id)} />
      <MacroPatterns report={app.macro_report} {connected} />
      <DeathMapPanel games={app.history} />
      <Trends trends={app.trends} settings={app.settings} />
      <Lookup />
    </div>
  </div>
{/if}

<style>
  .idle {
    display: grid;
    grid-template-columns: minmax(320px, 2fr) minmax(0, 3fr);
    gap: var(--gap);
    align-items: start;
  }
  .left,
  .right {
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
