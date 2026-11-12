<script lang="ts">
  // The largest changes of the win chance in a reviewed game, and what happened in each.
  import type { Swing } from '../../lib/bindings/Swing';
  import { clockText, signed } from '../../lib/ui';

  let { swings }: { swings: Swing[] } = $props();
</script>

{#if swings.length}
  <section class="panel">
    <div class="top">
      <h2>Biggest swings</h2>
      <span class="muted small">win chance, points</span>
    </div>
    <table>
      <tbody>
        {#each swings as s, i (i)}
          <tr>
            <td class="num t">{clockText(s.time)}</td>
            <td class="num d" class:good={s.delta > 0} class:bad={s.delta < 0}>{signed(Math.round(s.delta))}</td>
            <td class:muted={s.kind === 'other'}>{s.text}</td>
          </tr>
        {/each}
      </tbody>
    </table>
    <p class="source">
      From the scoreboard, deaths aside. Read off the timeline once a minute, gold spent standing in for item gold and the
      champions' scaling not counted; games the coach watched use its live number.
    </p>
  </section>
{/if}

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
  td {
    padding: 2px 8px 2px 0;
    vertical-align: baseline;
  }
  .t {
    color: var(--muted);
    width: 44px;
    white-space: nowrap;
  }
  .d {
    width: 40px;
    font-size: 16px;
    font-weight: 600;
    text-align: right;
    padding-right: 12px;
  }
  .source {
    margin: 8px 0 0;
    font-size: 12px;
    color: var(--faint);
  }
</style>
