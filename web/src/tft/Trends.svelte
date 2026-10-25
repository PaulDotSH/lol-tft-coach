<script lang="ts">
  // TFT trends per mode (Standard and Choncc kept apart): average placement, top-4 rate
  // and the comps I played with their results.
  import type { TftTrends } from '../lib/tft-bindings/TftTrends';
  import { avgText, MODE_NAMES, share } from '../lib/tft';

  let { trends }: { trends: TftTrends[] } = $props();

  const shown = $derived(trends.filter((t) => t.games > 0));
</script>

<section class="panel">
  <h2>TFT trends</h2>
  {#if !shown.length}
    <p class="empty">Trends appear after a few TFT games.</p>
  {:else}
    {#each shown as t (t.mode)}
      <h3>{MODE_NAMES[t.mode]} <span class="faint">{t.games} games</span></h3>
      <div class="kpis">
        <div><span class="num big">{avgText(t.avg_place)}</span><span class="muted">avg placement</span></div>
        <div><span class="num big">{share(t.top4)}</span><span class="muted">top 4</span></div>
      </div>
      {#if t.comps.length}
        <table>
          <thead>
            <tr><th>Comp</th><th class="r">Games</th><th class="r">Avg</th><th class="r">Top 4</th></tr>
          </thead>
          <tbody>
            {#each t.comps as c (c.comp)}
              <tr>
                <td class="cname">{c.name}</td>
                <td class="r num">{c.games}</td>
                <td class="r num" class:good={c.avg_place <= 4} class:bad={c.avg_place > 4.5}>{avgText(c.avg_place)}</td>
                <td class="r num">{share(c.top4)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    {/each}
  {/if}
</section>

<style>
  h3 {
    margin: 14px 0 6px;
    font: 600 14px var(--cond);
    color: var(--muted);
  }
  h3:first-of-type {
    margin-top: 0;
  }
  .faint {
    color: var(--faint);
    font-weight: 400;
  }
  .kpis {
    display: flex;
    gap: 28px;
    margin-bottom: 8px;
  }
  .kpis div {
    display: grid;
    font-size: 13px;
  }
  .big {
    font-size: 28px;
    font-weight: 600;
    line-height: 1.05;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 14px;
  }
  th {
    font: 600 13px var(--cond);
    color: var(--faint);
    text-align: left;
    padding: 2px 6px;
  }
  td {
    padding: 3px 6px;
    border-top: 1px solid var(--line);
  }
  .r {
    text-align: right;
  }
  td.num {
    font-size: 15px;
  }
  .cname {
    font: 600 15px var(--cond);
  }
</style>
