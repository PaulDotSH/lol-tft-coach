<script lang="ts">
  // How the game goes by length: early / mid / late edge between the teams, the scaling champions
  // under their phase, then the comp, spike, lane and record lines. Full while loading and for the
  // first PLAN_SHOW_S of the game (`show_s`), then one line that expands on click.
  import type { GamePlan } from '../lib/bindings/GamePlan';
  import type { PlanPhase } from '../lib/bindings/PlanPhase';
  import type { Scaler } from '../lib/bindings/Scaler';
  import { clock, store } from '../lib/store.svelte';
  import { champName } from '../lib/ui';
  import Icon from './Icon.svelte';

  let { plan, full = false }: { plan: GamePlan; full?: boolean } = $props();
  let open = $state(false);

  const expanded = $derived(full || open || clock.time < plan.show_s);
  const name = (k: number) => (store.ui ? champName(store.ui, k) : '');
  const span = (p: PlanPhase) =>
    p.to_min == null ? `${p.from_min}+ min` : p.from_min === 0 ? `under ${p.to_min} min` : `${p.from_min}–${p.to_min} min`;
  const side = (p: PlanPhase) => (p.even ? 'even' : p.edge > 0 ? 'you' : 'them');
  const value = (p: PlanPhase) => (p.even ? 'Even' : `${p.edge > 0 ? 'You' : 'Them'} +${Math.abs(p.edge).toFixed(1)}`);
  const late = (s: Scaler) => s.late >= s.early;
  const under = (i: number) => plan.scalers.filter((s) => (i === 2 ? late(s) : i === 0 ? !late(s) : false));
  const tip = (s: Scaler) =>
    `${name(s.champ)}: ${s.early >= 0 ? '+' : '−'}${Math.abs(s.early).toFixed(1)} early, ${s.late >= 0 ? '+' : '−'}${Math.abs(s.late).toFixed(1)} late` +
    (s.off_lane ? ' (main-lane data: few games in this lane)' : '');
</script>

{#if expanded}
  <section class="panel plan" aria-label="Game plan">
    <div class="head">
      <h2>Game plan</h2>
      {#if open}<button class="link" onclick={() => (open = false)}>Fold</button>{/if}
    </div>
    <ol class="bar">
      {#each plan.phases as p, i (p.name)}
        <li class={side(p)}>
          <div class="seg">
            <span class="phase">{p.name} <span class="win">{span(p)}</span></span>
            <span class="num pts">{value(p)}</span>
          </div>
          <div class="scalers">
            {#each under(i) as s (s.champ)}
              <span class="scaler" class:enemy={!s.ally} title={tip(s)}>
                <Icon kind="champ" id={s.champ} size={28} title={tip(s)} />
                {#if s.off_lane}<span class="mark">*</span>{/if}
              </span>
            {/each}
          </div>
        </li>
      {/each}
    </ol>
    <ul class="lines">
      {#each plan.lines as l (l.kind)}<li class={l.kind}>{l.text}</li>{/each}
    </ul>
    <p class="note">
      Win rate by game length: how each champion does in games that end in that window, against its own average.
      Short games are full of stomps, so only the difference between the teams counts.
      {plan.source}{plan.partial ? ` · ${plan.curves} of ${plan.champs} champions` : ''}
    </p>
  </section>
{:else}
  <button class="panel fold" onclick={() => (open = true)} aria-label="Game plan, click to expand">
    <span class="label">Game plan</span>
    <span>{plan.fold}</span>
    <span class="more muted">▸</span>
  </button>
{/if}

<style>
  .plan {
    display: grid;
    gap: 10px;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }
  .head h2 {
    margin: 0;
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    color: var(--muted);
    cursor: pointer;
    font-size: 13px;
  }
  .bar {
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 4px;
  }
  .seg {
    display: grid;
    gap: 2px;
    padding: 6px 10px;
    background: var(--raised);
    border-bottom: 3px solid var(--line);
  }
  li:first-child .seg {
    border-radius: var(--r) 0 0 var(--r);
  }
  li:last-child .seg {
    border-radius: 0 var(--r) var(--r) 0;
  }
  .you .seg {
    border-bottom-color: var(--good);
  }
  .them .seg {
    border-bottom-color: var(--threat);
  }
  .phase {
    font: 600 13px/1.2 var(--cond);
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .win {
    text-transform: none;
    letter-spacing: 0;
    color: var(--faint);
  }
  .pts {
    font-size: 20px;
    font-weight: 600;
  }
  .you .pts {
    color: var(--good);
  }
  .them .pts {
    color: var(--threat);
  }
  .even .pts {
    color: var(--muted);
  }
  .scalers {
    display: flex;
    gap: 6px;
    min-height: 34px;
    padding: 4px 2px 0;
  }
  .scaler {
    position: relative;
  }
  .scaler :global(.icon) {
    box-shadow: 0 0 0 2px var(--good);
  }
  .scaler.enemy :global(.icon) {
    box-shadow: 0 0 0 2px var(--threat);
  }
  .mark {
    position: absolute;
    right: -4px;
    top: -4px;
    color: var(--warn);
    font-weight: 700;
  }
  .lines {
    margin: 0;
    padding: 0 0 0 18px;
    display: grid;
    gap: 4px;
  }
  .scaling {
    color: var(--text);
    font-weight: 500;
  }
  .note {
    margin: 0;
    font-size: 12px;
    color: var(--faint);
  }
  .fold {
    display: flex;
    gap: 10px;
    align-items: baseline;
    width: 100%;
    border: 0;
    text-align: left;
    cursor: pointer;
    padding: 6px 12px;
  }
  .label {
    font: 600 13px/1.2 var(--cond);
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .more {
    margin-left: auto;
  }
</style>
