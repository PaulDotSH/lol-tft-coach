<script lang="ts">
  // My lane's lolalytics tier list: counters to my lane opponent ranked by win rate against
  // them, and the best champions in the lane in general.
  import type { TierBoard } from '../../lib/bindings/TierBoard';
  import type { TierPick } from '../../lib/bindings/TierPick';
  import { store } from '../../lib/store.svelte';
  import { LANE_NAMES, champName, gold, pct } from '../../lib/ui';
  import Icon from '../Icon.svelte';

  let { board }: { board: TierBoard } = $props();
  const name = (c: number) => (store.ui ? champName(store.ui, c) : '');
  const tierClass = (t: string | null) => (t ? (t.startsWith('S') ? 's' : t.startsWith('A') ? 'a' : t.startsWith('B') ? 'b' : 'c') : 'c');
  const vsName = $derived(board.vs != null ? name(board.vs) : '');
</script>

{#snippet row(p: TierPick, vs: boolean)}
  <li class:mine={p.mine}>
    <span class="pic">
      <Icon kind="champ" id={p.champ} size={34} />
      <span class="tier num {tierClass(p.tier)}">{p.tier ?? '?'}</span>
    </span>
    <span class="who">
      <span class="name">{name(p.champ)}{#if p.mine}<span class="you">you</span>{/if}</span>
      <span class="muted sub num">
        {#if vs}{pct(p.win_rate)} in lane{:else}{pct(p.pick_rate)} pick{#if p.vs_wr != null}{` · ${pct(p.vs_wr)} vs ${vsName}`}{/if}{/if}
      </span>
    </span>
    <span class="num big" class:good={vs && (p.vs_wr ?? 50) >= 52} class:bad={vs && (p.vs_wr ?? 50) < 48}>
      {pct(vs ? (p.vs_wr ?? 0) : p.win_rate)}
      {#if vs}<span class="games">{gold(p.vs_games)}</span>{/if}
    </span>
  </li>
{/snippet}

<section class="panel board">
  <div class="col">
    <h2>{board.vs != null ? `Into ${vsName}` : 'Counters'}</h2>
    {#if board.vs == null}
      <p class="empty">Shown once your lane opponent is revealed.</p>
    {:else if !board.counters.length}
      <p class="empty">No matchup data against {vsName} yet.</p>
    {:else}
      <ol>{#each board.counters as p (p.champ)}{@render row(p, true)}{/each}</ol>
    {/if}
  </div>
  <div class="col">
    <h2>Best in {LANE_NAMES[board.lane]}</h2>
    <ol>{#each board.best as p (p.champ)}{@render row(p, false)}{/each}</ol>
  </div>
  <p class="note">lolalytics tier list for your lane; banned and picked champions left out. Win rate against them, with games.</p>
</section>

<style>
  .board {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 6px 20px;
  }
  ol {
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    gap: 4px;
  }
  li {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    padding: 3px 8px 3px 3px;
    border-radius: var(--r);
  }
  .mine {
    background: var(--raised);
  }
  .mine :global(.icon) {
    box-shadow: 0 0 0 2px var(--text);
  }
  .pic {
    position: relative;
    line-height: 0;
  }
  .tier {
    position: absolute;
    right: -6px;
    bottom: -4px;
    min-width: 22px;
    padding: 0 3px;
    border-radius: 4px;
    font-size: 12px;
    font-weight: 700;
    line-height: 1.3;
    text-align: center;
    background: var(--line);
  }
  .s {
    background: var(--gold);
    color: #2a2210;
  }
  .a {
    background: var(--good);
    color: #10261f;
  }
  .b {
    background: #6b7f96;
  }
  .who {
    display: grid;
    min-width: 0;
  }
  .name {
    font: 600 18px/1.1 var(--cond);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .you {
    margin-left: 6px;
    font: 500 12px/1 var(--sans);
    color: var(--muted);
  }
  .sub {
    font-size: 13px;
  }
  .big {
    font-size: 18px;
    font-weight: 600;
    text-align: right;
  }
  .games {
    display: block;
    font-size: 12px;
    font-weight: 500;
    color: var(--faint);
  }
  .note {
    grid-column: 1 / -1;
    margin: 6px 0 0;
    font-size: 12px;
    color: var(--faint);
  }
  @media (max-width: 760px) {
    .board {
      grid-template-columns: 1fr;
    }
  }
</style>
