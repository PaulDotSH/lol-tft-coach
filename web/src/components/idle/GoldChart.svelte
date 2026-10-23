<script lang="ts">
  // Team and lane gold lead over the game, with kills below and objectives above.
  import type { GoldPoint } from '../../lib/bindings/GoldPoint';
  import type { KillMark } from '../../lib/bindings/KillMark';
  import type { TimelineMark } from '../../lib/bindings/TimelineMark';
  import { clockText, gold, signed } from '../../lib/ui';

  let {
    points,
    kills,
    marks,
    duration,
    opponent,
  }: { points: GoldPoint[]; kills: KillMark[]; marks: TimelineMark[]; duration: number; opponent: string | null } = $props();

  const W = 640;
  const H = 200;
  const X0 = 40;
  const X1 = W - 8;
  const Y0 = 24;
  const Y1 = 160;
  const MID = (Y0 + Y1) / 2;
  const KILL_Y = 172;

  const span = $derived(Math.max(duration, points.at(-1)?.time ?? 0, 60));
  const max = $derived(
    Math.ceil(Math.max(1000, ...points.map((p) => Math.max(Math.abs(p.team), Math.abs(p.lane ?? 0)))) / 1000) * 1000,
  );
  const x = (t: number) => X0 + (t / span) * (X1 - X0);
  const y = (v: number) => MID - (v / max) * (MID - Y0);
  const line = (vals: [number, number][]) => vals.map(([t, v], i) => `${i ? 'L' : 'M'}${x(t).toFixed(1)},${y(v).toFixed(1)}`).join('');
  const team = $derived(line(points.map((p) => [p.time, p.team])));
  const area = $derived(points.length ? `${team}L${x(points.at(-1)!.time)},${MID}L${x(points[0].time)},${MID}Z` : '');
  const lane = $derived(points.some((p) => p.lane != null) ? line(points.map((p) => [p.time, p.lane ?? 0])) : '');
  const ticks = $derived(Array.from({ length: Math.floor(span / 300) + 1 }, (_, i) => i * 300));

  const LETTER: Record<string, string> = { Dragon: 'D', Baron: 'B', Herald: 'H', Grubs: 'G', Atakhan: 'A', Tower: 'T', Inhibitor: 'I' };
  const minor = (m: TimelineMark) => m.kind === 'Tower';
  const markTitle = (m: TimelineMark) =>
    `${clockText(m.time)} ${m.ours ? 'Your team' : 'They'} took ${m.kind}${m.detail ? ` (${m.detail})` : ''}`;

  let hover = $state<GoldPoint | null>(null);
  function move(e: PointerEvent) {
    const r = (e.currentTarget as SVGElement).closest('svg')!.getBoundingClientRect();
    const t = ((((e.clientX - r.left) / r.width) * W - X0) / (X1 - X0)) * span;
    hover = points.reduce<GoldPoint | null>((b, p) => (!b || Math.abs(p.time - t) < Math.abs(b.time - t) ? p : b), null);
  }
</script>

<div class="chart">
  <div class="legend small">
    <span><i class="sw team"></i>Team gold lead</span>
    {#if lane}<span><i class="sw lane"></i>Lane{opponent ? ` vs ${opponent}` : ''}</span>{/if}
    <span class="read num">
      {#if hover}
        {clockText(hover.time)} · team {signed(hover.team, gold)}{hover.lane != null ? ` · lane ${signed(hover.lane, gold)}` : ''}
      {/if}
    </span>
  </div>
  <svg viewBox="0 0 {W} {H}" role="img" aria-label="Gold lead over the game">
    <defs>
      <clipPath id="above"><rect x={X0} y={Y0 - 2} width={X1 - X0} height={MID - Y0 + 2} /></clipPath>
      <clipPath id="below"><rect x={X0} y={MID} width={X1 - X0} height={Y1 - MID + 2} /></clipPath>
    </defs>
    {#each [max, max / 2, 0, -max / 2, -max] as v (v)}
      <line class="grid" class:zero={v === 0} x1={X0} x2={X1} y1={y(v)} y2={y(v)} />
      <text class="axis" x={X0 - 6} y={y(v) + 4} text-anchor="end">{v === 0 ? '0' : signed(v, gold)}</text>
    {/each}
    {#each ticks as t (t)}
      <text class="axis" x={x(t)} y={H - 4} text-anchor="middle">{t / 60}</text>
    {/each}

    <path class="fill good" d={area} clip-path="url(#above)" />
    <path class="fill bad" d={area} clip-path="url(#below)" />
    {#if lane}<path class="lane" d={lane} />{/if}
    <path class="team" d={team} />

    {#each marks as m, i (i)}
      <text class="mark" class:minor={minor(m)} class:ours={m.ours} x={x(m.time)} y={minor(m) ? 20 : 13} text-anchor="middle">
        <title>{markTitle(m)}</title>{LETTER[m.kind] ?? '•'}
      </text>
    {/each}
    {#each kills as k, i (i)}
      <circle class="kill" class:ours={k.ours} class:mine={k.mine} cx={x(k.time)} cy={KILL_Y} r={k.mine ? 4.5 : 3}>
        <title>{clockText(k.time)} {k.ours ? 'Your team got a kill' : 'Your team lost a champion'}{k.mine ? ' (you were in it)' : ''}</title>
      </circle>
    {/each}

    {#if hover}<line class="cursor" x1={x(hover.time)} x2={x(hover.time)} y1={Y0} y2={Y1} />{/if}
    <rect
      class="hit"
      x={X0}
      y={Y0}
      width={X1 - X0}
      height={Y1 - Y0}
      role="presentation"
      onpointermove={move}
      onpointerleave={() => (hover = null)}
    />
  </svg>
  <p class="key small muted">
    Top row: <b class="ours">D</b>ragon, <b class="ours">B</b>aron, <b class="ours">H</b>erald, <b class="ours">G</b>rubs,
    <b class="ours">T</b>ower, <b class="ours">I</b>nhibitor (green yours, red theirs). Dots: kills, large when you were in
    them. Minutes along the bottom.
  </p>
</div>

<style>
  .chart {
    display: grid;
    gap: 4px;
  }
  svg {
    width: 100%;
    height: auto;
    display: block;
    touch-action: none;
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 14px;
    align-items: center;
    color: var(--muted);
  }
  .read {
    margin-left: auto;
    color: var(--text);
    font-size: 14px;
  }
  .sw {
    display: inline-block;
    width: 16px;
    height: 0;
    margin-right: 5px;
    vertical-align: middle;
    border-top: 2px solid var(--gold);
  }
  .sw.lane {
    border-top: 2px dashed var(--muted);
  }
  .small {
    font-size: 13px;
  }
  .grid {
    stroke: var(--line);
    stroke-width: 1;
  }
  .grid.zero {
    stroke: var(--faint);
  }
  .axis {
    fill: var(--faint);
    font: 11px var(--cond);
  }
  .fill {
    stroke: none;
    opacity: 0.22;
  }
  .fill.good {
    fill: var(--good);
  }
  .fill.bad {
    fill: var(--threat);
  }
  .team {
    fill: none;
    stroke: var(--gold);
    stroke-width: 2;
    stroke-linejoin: round;
  }
  .lane {
    fill: none;
    stroke: var(--muted);
    stroke-width: 1.5;
    stroke-dasharray: 4 3;
  }
  .mark {
    font: 700 12px var(--cond);
    fill: var(--threat);
    cursor: default;
  }
  .mark.ours,
  .key .ours {
    fill: var(--good);
    color: var(--good);
  }
  .mark.minor {
    font-size: 9px;
    font-weight: 600;
    opacity: 0.75;
  }
  .kill {
    fill: var(--threat);
    opacity: 0.8;
  }
  .kill.ours {
    fill: var(--good);
  }
  .kill.mine {
    opacity: 1;
    stroke: var(--text);
    stroke-width: 1.2;
  }
  .cursor {
    stroke: var(--text);
    stroke-width: 1;
    opacity: 0.4;
  }
  .hit {
    fill: transparent;
  }
  .key {
    margin: 0;
  }
</style>
