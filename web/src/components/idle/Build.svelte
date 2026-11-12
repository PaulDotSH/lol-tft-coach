<script lang="ts">
  // My item times and first back beside the middle half of Emerald+ and Master+ games on my
  // champion and role, from the ranked games the minipc crawls. Times only, never win rates.
  import type { BuildReview } from '../../lib/bindings/BuildReview';
  import { store } from '../../lib/store.svelte';
  import { clockText, itemName } from '../../lib/ui';
  import Icon from '../Icon.svelte';

  let { b }: { b: BuildReview } = $props();

  const name = (id: number) => (store.ui ? itemName(store.ui, id) : `Item ${id}`);
  const span = (r: [number, number] | null) => (r ? `${clockText(r[0])}–${clockText(r[1])}` : '–');
  const late = (t: number, r: [number, number] | null) => r != null && t > r[1];
  const early = (t: number, r: [number, number] | null) => r != null && t < r[0];
  const usual = $derived(b.back?.usual);
</script>

<section class="panel">
  <div class="top">
    <h2>Build</h2>
    <span class="muted small">middle half of games</span>
  </div>
  <table>
    <thead>
      <tr>
        <th></th>
        <th class="num">You</th>
        <th class="num">Emerald+</th>
        {#if b.top_games != null}<th class="num">Master+</th>{/if}
      </tr>
    </thead>
    <tbody>
      {#each b.items as r, i (i)}
        <tr>
          <td class="what"><Icon kind="item" id={r.item} size={22} /><span><span class="muted">{r.slot}</span> {name(r.item)}</span></td>
          <td class="num" class:bad={late(r.time, r.all)} class:good={early(r.time, r.all)}>{clockText(r.time)}</td>
          <td class="num muted">{span(r.all)}</td>
          {#if b.top_games != null}<td class="num muted">{span(r.top)}</td>{/if}
        </tr>
      {/each}
      {#if b.back}
        <tr>
          <td class="what"><span><span class="muted">first back</span> {b.back.gold.toLocaleString()} gold</span></td>
          <td class="num" class:bad={late(b.back.time, b.back.all)}>{clockText(b.back.time)}</td>
          <td class="num muted" title={b.back.all_gold ? `with ${b.back.all_gold[0]}–${b.back.all_gold[1]} gold` : ''}>{span(b.back.all)}</td>
          {#if b.top_games != null}<td class="num muted">{span(b.back.top)}</td>{/if}
        </tr>
      {/if}
    </tbody>
  </table>
  {#if b.tips.length}
    <ul class="tips">
      {#each b.tips as t, i (i)}<li>{t}</li>{/each}
    </ul>
  {/if}
  <p class="source">
    Ranked games crawled on patch {b.patch}: {b.games.toLocaleString()} Emerald+{b.top_games != null ? `, ${b.top_games.toLocaleString()} Master+` : ''} on this champion and role.{usual != null
      ? ` First backs most often buy ${name(usual)}.`
      : ''}
    What to build stays Coachless'.
  </p>
</section>

<style>
  .top {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
  }
  .small {
    font-size: 13px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 14px;
  }
  th {
    font-weight: 400;
    color: var(--muted);
    font-size: 12px;
    text-align: right;
  }
  td {
    padding: 2px 0 2px 10px;
    vertical-align: middle;
  }
  td.num {
    text-align: right;
    white-space: nowrap;
  }
  .what {
    display: flex;
    align-items: center;
    gap: 6px;
    padding-left: 0;
  }
  .tips {
    margin: 8px 0 0;
    padding-left: 18px;
    display: grid;
    gap: 4px;
  }
  .source {
    margin: 8px 0 0;
    font-size: 12px;
    color: var(--faint);
  }
</style>
