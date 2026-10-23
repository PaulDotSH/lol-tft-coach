<script lang="ts">
  // This game against my recent games on the same champion and mode.
  import type { PastCompare } from '../../lib/bindings/PastCompare';

  let { c, champ }: { c: PastCompare; champ: string } = $props();
  const fmt = (v: number, d: number) => (v > 0 && d === 0 && v >= 1000 ? Math.round(v).toString() : v.toFixed(d));
  const better = (this_: number, other: number, lower: boolean) => (lower ? this_ < other : this_ > other);
</script>

<section class="panel">
  <h2>Against your last {c.games} {champ} games <span class="faint">({c.wins}W {c.games - c.wins}L)</span></h2>
  <table>
    <thead><tr><th></th><th>This game</th><th>Your usual</th><th>Your best</th></tr></thead>
    <tbody>
      {#each c.rows as r (r.label)}
        <tr>
          <th>{r.label}</th>
          <td class="num" class:good={better(r.this, r.avg, r.lower_better)} class:bad={better(r.avg, r.this, r.lower_better)}>
            {fmt(r.this, r.digits)}{#if better(r.this, r.best, r.lower_better) || r.this === r.best}<span class="star" title="Your best so far">★</span>{/if}
          </td>
          <td class="num muted">{fmt(r.avg, r.digits)}</td>
          <td class="num muted">{fmt(r.best, r.digits)}</td>
        </tr>
      {/each}
    </tbody>
  </table>
</section>

<style>
  h2 .faint {
    color: var(--faint);
    font-weight: 400;
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  thead th {
    font: 600 12px var(--cond);
    color: var(--faint);
    text-align: right;
    padding-bottom: 4px;
  }
  tbody th {
    text-align: left;
    font-weight: 400;
    color: var(--muted);
  }
  td {
    text-align: right;
    font-size: 16px;
    padding: 1px 0 1px 12px;
  }
  .star {
    margin-left: 3px;
    color: var(--gold);
    font-size: 12px;
  }
</style>
