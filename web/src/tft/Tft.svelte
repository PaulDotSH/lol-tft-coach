<script lang="ts">
  // The TFT screens, shown instead of the LoL views while a TFT phase is on: live advice in
  // game; top comps in the lobby; the review after a game; history and trends between games.
  import type { TftState } from '../lib/tft-bindings/TftState';
  import { MODE_NAMES, stageText, tftQueueName } from '../lib/tft';
  import InGame from './ingame/InGame.svelte';
  import History from './History.svelte';
  import Lobby from './Lobby.svelte';
  import Review from './Review.svelte';
  import Status from './Status.svelte';
  import Trends from './Trends.svelte';

  let { state }: { state: TftState } = $props();

  const PHASES = { none: '', lobby: 'Lobby', ingame: 'In game', postgame: 'Game over' } as const;
  const queue = $derived(tftQueueName(state.queue, state.mode));
  const stage = $derived(state.advice ? stageText(state.advice.stage) : '');

  // Comp ids → names for history rows, from every list that carries both.
  const names = $derived.by(() => {
    const m = new Map<string, string>();
    for (const c of state.meta) m.set(c.comp, c.name);
    for (const t of state.trends) for (const c of t.comps) m.set(c.comp, c.name);
    if (state.review?.comp) m.set(state.review.comp.comp, state.review.comp.name);
    return m;
  });
</script>

<div class="tft">
  <p class="title">
    <span class="tag">TFT</span>
    <span class="phase">{PHASES[state.phase]}</span>
    <span class="muted">{queue}</span>
    {#if state.mode !== 'standard' && queue !== MODE_NAMES[state.mode]}<span class="muted">· {MODE_NAMES[state.mode]}</span>{/if}
    {#if state.phase === 'ingame' && stage}<span class="num muted">Stage {stage}</span>{/if}
  </p>

  {#if state.phase === 'ingame'}
    {#if state.advice}
      <InGame advice={state.advice} />
    {:else}
      <p class="empty">Reading the screen…</p>
    {/if}
  {:else}
    <div class="between">
      <div class="left">
        {#if state.phase === 'postgame' && state.review}
          <Review review={state.review} />
        {:else if state.phase === 'postgame'}
          <section class="panel"><h2>Last game</h2><p class="empty">Waiting for the match history…</p></section>
        {/if}
        {#if state.phase === 'lobby' || state.meta.length}
          <Lobby meta={state.meta} mode={state.mode} {queue} />
        {/if}
      </div>
      <div class="right">
        <History games={state.history} {names} />
        <Trends trends={state.trends} />
      </div>
    </div>
  {/if}

  <Status status={state.status} />
</div>

<style>
  .tft {
    display: flex;
    flex-direction: column;
    gap: var(--gap);
  }
  .title {
    margin: 0;
    display: flex;
    align-items: baseline;
    gap: 10px;
    font: 600 17px var(--cond);
  }
  .tag {
    padding: 0 6px;
    border-radius: 4px;
    background: var(--gold);
    color: var(--bg);
    font-weight: 700;
  }
  .phase {
    font-size: 20px;
  }
  .between {
    display: grid;
    grid-template-columns: minmax(320px, 3fr) minmax(0, 2fr);
    gap: var(--gap);
    align-items: start;
  }
  .left,
  .right {
    display: grid;
    gap: var(--gap);
    min-width: 0;
  }
  @media (max-width: 760px) {
    .between {
      grid-template-columns: 1fr;
    }
  }
</style>
