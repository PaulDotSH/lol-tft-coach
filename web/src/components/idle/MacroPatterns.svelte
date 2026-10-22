<script lang="ts">
  // "Your patterns": what separates recent losses from wins (macro coach).
  import type { MacroReport } from '../../lib/bindings/MacroReport';
  import type { Mode } from '../../lib/bindings/Mode';
  import Icon from '../Icon.svelte';

  let { report, connected }: { report: MacroReport | null; connected: boolean } = $props();

  const SHORT: Record<Mode, string> = { rift: 'Rift', aram: 'ARAM', mayhem: 'Mayhem' };
  const fmt = (v: number, unit: string) =>
    unit === '%' ? `${v.toFixed(0)}%` : unit === '/min' && v >= 20 ? v.toFixed(0) : v.toFixed(1);
  const max = (bars: [string, number][]) => Math.max(...bars.map(([, v]) => v), 1e-6);
</script>

<section class="panel">
  <div class="top">
    <h2>Your patterns</h2>
    {#if report}<span class="muted small">last {report.games} games</span>{/if}
  </div>

  {#if !report}
    <p class="empty">{connected ? 'Reading your match history…' : 'Connect the League client to read your recent games.'}</p>
  {:else}
    <div class="modes">
      {#each report.modes as m (m.mode)}
        <span class="mode"><b>{SHORT[m.mode]}</b> <span class="num">{m.wins}W {m.games - m.wins}L</span></span>
      {/each}
    </div>

    {#if !report.patterns.length}
      <p class="empty">
        No clear pattern yet. Each mode needs about 8 games, with at least 4 wins and 4 losses, before wins and losses can be
        compared.
      </p>
    {:else}
      <ol>
        {#each report.patterns as p (`${p.mode}-${p.kind}-${p.champ}`)}
          <li>
            <div class="head">
              {#if p.champ != null}<Icon kind="champ" id={p.champ} size={26} />{/if}
              <span class="title">{p.title}</span>
              <span class="tag">{SHORT[p.mode]}</span>
            </div>
            <p class="evidence">{p.evidence}</p>
            <div class="bars">
              {#each p.bars as [label, v], i (label)}
                <span class="label muted">{label}</span>
                <span class="track"><span class="fill" class:first={i === 0} class:win={label === 'Wins'} style:width="{(100 * v) / max(p.bars)}%"></span></span>
                <span class="num val">{fmt(v, p.unit)}{p.unit === '%' ? '' : ` ${p.unit}`}</span>
              {/each}
            </div>
            <p class="focus muted">{p.focus}</p>
          </li>
        {/each}
      </ol>
    {/if}
    {#if report.games && report.with_timeline < report.games}
      <p class="note">{report.games - report.with_timeline} games had no timeline: no death times or gold at 15 for those.</p>
    {/if}
  {/if}
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
  .modes {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 10px;
  }
  .mode {
    padding: 1px 8px;
    border-radius: 999px;
    background: var(--raised);
    font-size: 13px;
  }
  .mode b {
    font-weight: 500;
    color: var(--muted);
  }
  ol {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
    gap: 10px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    padding: 10px 12px;
    border-radius: var(--r);
    border-left: 3px solid var(--threat);
    background: var(--raised);
  }
  li:first-child {
    border-left-color: var(--gold);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .title {
    font: 600 19px/1.1 var(--cond);
  }
  .tag {
    margin-left: auto;
    font: 600 12px var(--cond);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--faint);
  }
  .evidence {
    margin: 4px 0 6px;
  }
  .bars {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 3px 8px;
    font-size: 13px;
  }
  .track {
    height: 6px;
    border-radius: 3px;
    background: var(--panel);
    overflow: hidden;
  }
  .fill {
    display: block;
    height: 100%;
    border-radius: 3px;
    background: var(--faint);
  }
  .fill.first {
    background: var(--threat);
  }
  .fill.win {
    background: var(--good);
  }
  .val {
    min-width: 52px;
    text-align: right;
    font-size: 15px;
  }
  .focus {
    margin: 6px 0 0;
    font-size: 13px;
  }
  .note {
    margin: 10px 0 0;
    font-size: 12px;
    color: var(--faint);
  }
</style>
