<script lang="ts">
  // Item gold (cost of held items, as the scoreboard shows) — never enemy gold, which the game hides.
  import type { GoldDiff } from '../../lib/bindings/GoldDiff';
  import { LANE_NAMES, gold, signed } from '../../lib/ui';
  import { winPath } from '../../lib/winprob';
  import Icon from '../Icon.svelte';

  let { g, win = false }: { g: GoldDiff; win?: boolean } = $props();
  const uid = $props.id();
  const [mine, theirs] = $derived(g.team_item_gold);
  const share = $derived(mine + theirs > 0 ? (100 * mine) / (mine + theirs) : 50);
  const tone = (n: number) => (n > 0 ? 'good' : n < 0 ? 'bad' : '');
  const allies = $derived(g.players.filter((p) => p.ally));
  const enemies = $derived(g.players.filter((p) => !p.ally));

  // Sparkline: symmetric around 0, at least ±1k so early-game noise stays flat.
  const W = 200;
  const H = 56;
  const spark = $derived.by(() => {
    const h = g.history;
    if (h.length < 2) return null;
    const t0 = h[0].time;
    const span = Math.max(h[h.length - 1].time - t0, 1);
    const max = Math.max(1000, ...h.map((s) => Math.max(Math.abs(s.team), Math.abs(s.lane ?? 0))));
    const pt = (t: number, v: number) => `${(((t - t0) / span) * W).toFixed(1)},${(H / 2 - (v / max) * (H / 2 - 2)).toFixed(1)}`;
    const path = (pts: [number, number][]) => pts.map(([t, v], i) => `${i ? 'L' : 'M'}${pt(t, v)}`).join('');
    const team = path(h.map((s) => [s.time, s.team]));
    const lanePts = h.filter((s) => s.lane != null).map((s): [number, number] => [s.time, s.lane!]);
    return { team, area: `${team}L${W},${H / 2}L0,${H / 2}Z`, lane: lanePts.length > 1 ? path(lanePts) : null, win: win ? winPath(h, W, H) : null, max };
  });
</script>

<section class="panel row" aria-label="Item gold">
  <div class="team">
    <div class="head">
      <span class="big num {tone(g.diff)}">{signed(g.diff, gold)}</span>
      <h2 title="Cost of items held, as the scoreboard shows them (enemy gold is hidden by the game)">item gold</h2>
    </div>
    <div class="split num">
      <span>{gold(mine)}</span>
      <div class="bar" style:--share="{share}%"><span></span></div>
      <span>{gold(theirs)}</span>
    </div>
    <div class="meta num">
      <span>Kills <b>{g.kills[0]}–{g.kills[1]}</b></span>
      <span>Avg lvl <b class={tone(g.level_diff)}>{signed(g.level_diff, (x) => x.toFixed(1))}</b></span>
    </div>
  </div>

  {#if spark}
    <div class="spark">
      <svg viewBox="0 0 {W} {H}" preserveAspectRatio="none" role="img" aria-label="Item gold difference over the game">
        <defs>
          <clipPath id="{uid}-up"><rect width={W} height={H / 2} /></clipPath>
          <clipPath id="{uid}-down"><rect y={H / 2} width={W} height={H / 2} /></clipPath>
        </defs>
        <path class="area up" d={spark.area} clip-path="url(#{uid}-up)" />
        <path class="area down" d={spark.area} clip-path="url(#{uid}-down)" />
        <line class="zero" x1="0" x2={W} y1={H / 2} y2={H / 2} />
        {#if spark.lane}<path class="line lane" d={spark.lane} />{/if}
        {#if spark.win}<path class="line win" d={spark.win} />{/if}
        <path class="line" d={spark.team} />
      </svg>
      <div class="legend">
        <span>team</span>
        {#if spark.lane}<span class="lane">your lane</span>{/if}
        {#if spark.win}<span class="win" title="Win chance, 50% on the line">win %</span>{/if}
        <span class="scale num">±{gold(spark.max)}</span>
      </div>
    </div>
  {/if}

  {#if g.lanes.length}
    <ul class="lanes">
      {#each g.lanes as l (l.ally)}
        <li class:mine={l.mine}>
          <span class="lname">{l.lane ? LANE_NAMES[l.lane] : '—'}</span>
          <span class="pair"><Icon kind="champ" id={l.ally} size={24} /><Icon kind="champ" id={l.enemy} size={24} /></span>
          <span class="num d {tone(l.gold_diff)}">{signed(l.gold_diff, gold)}</span>
          {#if l.level_diff}<span class="num lv {tone(l.level_diff)}">{signed(l.level_diff)} lvl</span>{/if}
        </li>
      {/each}
    </ul>
  {:else if g.players.length}
    <div class="players">
      {#each [allies, enemies] as side, i (i)}
        <ul class:enemy={i === 1}>
          {#each side as p (p.champ)}
            <li class:mine={p.mine}><Icon kind="champ" id={p.champ} size={24} /><span class="num">{gold(p.item_gold)}</span></li>
          {/each}
        </ul>
      {/each}
    </div>
  {/if}
</section>

<style>
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px 24px;
  }
  .team {
    flex: 0 1 220px;
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
  .split {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 6px;
    font-size: 15px;
    color: var(--muted);
  }
  .bar {
    height: 6px;
    border-radius: 3px;
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
  .meta {
    display: flex;
    gap: 14px;
    font-size: 15px;
    color: var(--muted);
  }
  .meta b {
    font-weight: 600;
    color: var(--text);
  }
  .meta b.good {
    color: var(--good);
  }
  .meta b.bad {
    color: var(--threat);
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
  .zero {
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
  .line.lane {
    stroke: var(--gold);
    stroke-dasharray: 3 2;
    opacity: 0.85;
  }
  .line.win {
    stroke: var(--muted);
    stroke-dasharray: 1 2.5;
    stroke-width: 2;
  }
  .legend {
    display: flex;
    gap: 10px;
    font-size: 12px;
    color: var(--faint);
  }
  .legend .lane {
    color: var(--gold-dim);
  }
  .scale {
    margin-left: auto;
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .lanes {
    flex: 1 1 auto;
    display: grid;
    grid-template-columns: repeat(5, minmax(64px, 1fr));
    gap: 6px;
  }
  .lanes li {
    display: grid;
    justify-items: center;
    gap: 1px;
    padding: 3px 4px;
    border-radius: 4px;
  }
  .lanes li.mine {
    background: var(--raised);
    box-shadow: inset 0 0 0 1px var(--gold-dim);
  }
  .lname {
    font: 600 12px/1.1 var(--cond);
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .pair {
    display: flex;
    gap: 2px;
  }
  .pair :global(.icon:last-child) {
    opacity: 0.8;
  }
  .d {
    font-size: 18px;
    font-weight: 600;
    line-height: 1.1;
  }
  .lv {
    font-size: 12px;
    line-height: 1;
  }
  .players {
    flex: 1 1 auto;
    display: grid;
    gap: 4px;
  }
  .players ul {
    display: grid;
    grid-template-columns: repeat(5, minmax(60px, 1fr));
    gap: 4px;
    padding-left: 6px;
    border-left: 2px solid var(--good);
  }
  .players ul.enemy {
    border-left-color: var(--threat);
  }
  .players li {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 1px 4px;
    border-radius: 4px;
    font-size: 16px;
  }
  .players .enemy li {
    color: var(--muted);
  }
  .players li.mine {
    background: var(--raised);
    box-shadow: inset 0 0 0 1px var(--gold-dim);
  }
</style>
