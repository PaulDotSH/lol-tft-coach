<script lang="ts">
  // My team's chance to win from what the scoreboard shows: the number, its trend over the game,
  // what moves it and what the next objectives are worth.
  import type { GoldSample } from '../../lib/bindings/GoldSample';
  import type { WinProb } from '../../lib/bindings/WinProb';
  import { signed } from '../../lib/ui';
  import { trendMark, winPath } from '../../lib/winprob';

  let { w, history }: { w: WinProb; history: GoldSample[] } = $props();
  const uid = $props.id();
  const pc = (x: number) => Math.round(x * 100);
  const tone = (n: number) => (n > 0 ? 'good' : n < 0 ? 'bad' : '');
  const lead = $derived(pc(w.p) - 50);
  const standing = $derived(Math.abs(pc(w.p) - pc(w.base)) >= 2);
  const swingMax = $derived(Math.max(10, ...w.stakes.map((s) => Math.abs(s.swing))));

  const W = 200;
  const H = 56;
  const half = $derived(Math.max(0.2, Math.ceil(20 * Math.max(...history.map((s) => (s.win == null ? 0 : Math.abs(s.win - 0.5))))) / 20));
  const line = $derived(winPath(history, W, H, half));
  const area = $derived(line && `${line}V${H / 2}H${line.slice(1, line.indexOf(','))}Z`);
</script>

<section class="panel win" aria-label="Win chance">
  <div class="row">
    <div class="now">
      <div class="head">
        <span class="big num {tone(lead)}">{w.rough ? '~' : ''}{pc(w.p)}%</span>
        <h2 title="From the scoreboard: item gold, levels, kills, objectives and who is dead. Fog, waves and cooldowns are not seen.">win chance</h2>
        {#if w.trend != null}
          <span class="trend num {tone(Math.round(w.trend))}" title="Change over the last two minutes, deaths aside">{trendMark(w.trend)} {signed(Math.round(w.trend))} in 2 min</span>
        {/if}
      </div>
      <div class="split">
        <span>you</span>
        <div class="bar" style:--share="{pc(w.p)}%"><span></span></div>
        <span>them</span>
      </div>
      {#if standing}<p class="base num">{pc(w.base)}% once everyone is back</p>{/if}
    </div>
    {#if line}
      <div class="spark">
        <svg viewBox="0 0 {W} {H}" preserveAspectRatio="none" role="img" aria-label="Win chance over the game">
          <defs>
            <clipPath id="{uid}-up"><rect width={W} height={H / 2} /></clipPath>
            <clipPath id="{uid}-down"><rect y={H / 2} width={W} height={H / 2} /></clipPath>
          </defs>
          <path class="area up" d={area} clip-path="url(#{uid}-up)" />
          <path class="area down" d={area} clip-path="url(#{uid}-down)" />
          <line class="mid" x1="0" x2={W} y1={H / 2} y2={H / 2} />
          <path class="line" d={line} />
        </svg>
        <div class="legend"><span>over the game</span><span class="scale num">50 ±{Math.round(half * 100)}%</span></div>
      </div>
    {/if}
  </div>

  {#if w.factors.length}
    <ul class="chips">
      {#each w.factors as f (f.key)}
        <li class={tone(Math.round(f.points))}>{f.label} <b class="num">{signed(Math.round(f.points))}</b></li>
      {/each}
    </ul>
  {/if}

  {#if w.stakes.length}
    <ul class="stakes">
      {#each w.stakes as s (s.name)}
        <li>
          <span class="sname">{s.name}{#if s.note}&nbsp;<small>· {s.note}</small>{/if}</span>
          <span class="sbar" style:--w="{(100 * Math.abs(s.swing)) / swingMax}%"><span></span></span>
          <span class="num swing">±{Math.round(Math.abs(s.swing))}</span>
        </li>
      {/each}
    </ul>
  {/if}

  <p class="source" class:rough={w.rough}>{w.rough ? 'Rough: ' : ''}{w.source}</p>
</section>

<style>
  .win {
    display: grid;
    gap: 8px;
    align-content: start;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 20px;
  }
  .now {
    flex: 1 1 220px;
    display: grid;
    gap: 3px;
  }
  .head {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .head h2 {
    margin: 0;
  }
  .big {
    font-size: 30px;
    font-weight: 600;
    line-height: 1.05;
  }
  .trend {
    margin-left: auto;
    font-size: 15px;
    color: var(--muted);
    white-space: nowrap;
  }
  .trend.good {
    color: var(--good);
  }
  .trend.bad {
    color: var(--threat);
  }
  .split {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 6px;
    font: 500 13px var(--cond);
    color: var(--muted);
  }
  .bar {
    position: relative;
    height: 8px;
    border-radius: 4px;
    background: var(--threat);
    overflow: hidden;
  }
  .bar span {
    display: block;
    height: 100%;
    width: var(--share);
    background: var(--good);
    box-shadow: 1px 0 0 var(--panel);
    transition: width 0.4s ease-out;
  }
  .bar::after {
    content: '';
    position: absolute;
    left: 50%;
    top: 0;
    bottom: 0;
    width: 2px;
    margin-left: -1px;
    background: var(--panel);
  }
  .base {
    margin: 0;
    font-size: 13px;
    color: var(--muted);
  }
  .spark {
    flex: 1 1 160px;
    min-width: 140px;
    max-width: 320px;
  }
  svg {
    display: block;
    width: 100%;
    height: 56px;
  }
  .area.up {
    fill: rgb(95 196 169 / 0.22);
  }
  .area.down {
    fill: rgb(228 104 93 / 0.22);
  }
  .mid {
    stroke: var(--line);
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
  }
  .line {
    fill: none;
    stroke: var(--text);
    stroke-width: 1.5;
    stroke-linejoin: round;
    vector-effect: non-scaling-stroke;
  }
  .legend {
    display: flex;
    gap: 10px;
    font-size: 12px;
    color: var(--faint);
  }
  .scale {
    margin-left: auto;
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 6px;
  }
  .chips li {
    padding: 1px 8px;
    border-radius: 10px;
    background: var(--raised);
    font-size: 14px;
    color: var(--muted);
  }
  .chips b {
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
  }
  .chips .good b {
    color: var(--good);
  }
  .chips .bad b {
    color: var(--threat);
  }
  .stakes {
    display: grid;
    grid-template-columns: auto minmax(40px, 1fr) auto;
    align-items: center;
    gap: 3px 10px;
  }
  .stakes li {
    display: contents;
  }
  .sname {
    font-size: 15px;
  }
  .sname small {
    color: var(--muted);
    font-size: 13px;
  }
  .sbar {
    height: 6px;
    border-radius: 3px;
    background: var(--raised);
    overflow: hidden;
  }
  .sbar span {
    display: block;
    height: 100%;
    width: var(--w);
    background: var(--gold-dim);
  }
  .swing {
    font-size: 17px;
    font-weight: 600;
    text-align: right;
  }
  .source {
    margin: 0;
    font-size: 12px;
    color: var(--faint);
  }
  .source.rough {
    color: var(--warn);
  }
</style>
