<script lang="ts">
  // My lane's lolalytics tier list, each champion with its win rate against my lane opponent
  // once revealed (the ranking against them is the Lane column of the picks).
  import type { TierBoard } from '../../lib/bindings/TierBoard';
  import { store } from '../../lib/store.svelte';
  import { LANE_NAMES, champName, pct } from '../../lib/ui';
  import Icon from '../Icon.svelte';

  let { board }: { board: TierBoard } = $props();
  const name = (c: number) => (store.ui ? champName(store.ui, c) : '');
  const tierClass = (t: string | null) => (t ? (t.startsWith('S') ? 's' : t.startsWith('A') ? 'a' : t.startsWith('B') ? 'b' : 'c') : 'c');
  const vsName = $derived(board.vs != null ? name(board.vs) : '');
</script>

<section class="panel">
  <h2>Best in {LANE_NAMES[board.lane]}</h2>
  <ol class="board">
    {#each board.best as p (p.champ)}
      <li class:mine={p.mine}>
        <span class="pic">
          <Icon kind="champ" id={p.champ} size={34} />
          <span class="tier num {tierClass(p.tier)}">{p.tier ?? '?'}</span>
        </span>
        <span class="who">
          <span class="name">{name(p.champ)}{#if p.mine}<span class="you">you</span>{/if}</span>
          <span class="muted sub num">{pct(p.pick_rate)} pick{#if p.vs_wr != null}{` · ${pct(p.vs_wr)} vs ${vsName}`}{/if}</span>
        </span>
        <span class="num big">{pct(p.win_rate)}</span>
      </li>
    {/each}
  </ol>
  <p class="note">lolalytics tier list for your lane; banned and picked champions left out.</p>
</section>

<style>
  .board {
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 4px 20px;
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
  .note {
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
