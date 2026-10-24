<script lang="ts">
  // Ranked LP per queue: current rank, LP won or lost per game, the LP line and the streak.
  // The client keeps no LP history, so games show from when the app started tracking.
  import type { RankTrend } from '../../lib/bindings/RankTrend';

  let { ranked }: { ranked: RankTrend[] } = $props();

  const QUEUE: Record<string, string> = { solo: 'Solo/Duo', flex: 'Flex' };
  /** Chips shown, newest last. */
  const SHOWN = 10;
  const title = (t: string) => t.charAt(0) + t.slice(1).toLowerCase();
  const rank = (r: RankTrend) => `${title(r.tier)}${r.division === 'NA' ? '' : ` ${r.division}`} · ${r.lp} LP`;
  const signed = (d: number) => (d > 0 ? `+${d}` : `${d}`);
  const streak = (s: number) => (s >= 2 ? `${s} wins in a row` : s <= -2 ? `${-s} losses in a row` : '');

  const W = 120;
  const H = 28;
  function spark(series: number[]) {
    const lo = Math.min(...series);
    const span = Math.max(...series) - lo || 1;
    const step = W / (series.length - 1);
    return series.map((v, i) => `${(i * step).toFixed(1)},${(H - 2 - ((v - lo) / span) * (H - 4)).toFixed(1)}`).join(' ');
  }
</script>

<section class="panel">
  <h2>Ranked</h2>
  {#each ranked as r (r.queue)}
    <div class="queue">
      <div class="head">
        <span class="muted">{QUEUE[r.queue] ?? r.queue}</span>
        <b>{rank(r)}</b>
        {#if streak(r.streak)}<span class="small" class:good={r.streak > 0} class:bad={r.streak < 0}>{streak(r.streak)}</span>{/if}
        {#if r.games.length >= 2}
          {@const first = r.games[0].ladder - r.games[0].delta}
          <svg width={W} height={H} viewBox="0 0 {W} {H}" aria-hidden="true">
            <polyline points={spark([first, ...r.games.map((g) => g.ladder)])} class:up={r.games.at(-1)!.ladder >= first} />
          </svg>
        {/if}
      </div>
      {#if r.games.length}
        <div class="chips">
          {#each r.games.slice(-SHOWN) as g (g.time)}
            <span class="chip num" class:win={g.win}>{signed(g.delta)}</span>
          {/each}
        </div>
      {:else}
        <p class="empty small">LP changes show from your next ranked game in this queue.</p>
      {/if}
    </div>
  {/each}
</section>

<style>
  .queue + .queue {
    margin-top: 10px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .head svg {
    margin-left: auto;
  }
  .small {
    font-size: 13px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 6px;
  }
  .chip {
    min-width: 34px;
    padding: 1px 6px;
    border-radius: 4px;
    background: color-mix(in srgb, var(--threat) 22%, var(--raised));
    text-align: center;
    font-size: 13px;
  }
  .chip.win {
    background: color-mix(in srgb, var(--good) 22%, var(--raised));
  }
  polyline {
    fill: none;
    stroke: var(--threat);
    stroke-width: 1.5;
    stroke-linejoin: round;
  }
  polyline.up {
    stroke: var(--good);
  }
</style>
