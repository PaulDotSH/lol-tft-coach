<script lang="ts">
  // Laning up to 14:00: my duo against theirs in bot lane (else me against my opponent),
  // gold difference per minute with lane kills, levels and CS, and everyone's first back.
  import type { Laning } from '../../lib/bindings/Laning';
  import { store } from '../../lib/store.svelte';
  import { champName, clockText, gold, signed } from '../../lib/ui';
  import Icon from '../Icon.svelte';

  let { l, rich }: { l: Laning; rich: number[] } = $props();

  const name = (c: number) => (store.ui ? champName(store.ui, c) : '');
  const names = (cs: number[]) => cs.map(name).join(' + ');
  const W = 560;
  const H = 120;
  const MID = H / 2;
  const n = $derived(l.minutes.length);
  const max = $derived(Math.max(500, ...l.minutes.map((m) => Math.abs(m.gold_diff))));
  const x = (minute: number) => ((minute - 0.5) / n) * W;
  const bar = $derived(W / n - 6);
  const y = (v: number) => MID - (v / max) * (MID - 10);
  const killX = (t: number) => (t / 60 / n) * W;
</script>

<section class="panel">
  <h2>Laning: {names(l.allies)} vs {names(l.enemies)}</h2>
  <p class="verdict" class:good={l.verdict.startsWith('Won')} class:bad={l.verdict.startsWith('Lost')}>{l.verdict}</p>

  <svg viewBox="0 0 {W} {H + 16}" role="img" aria-label="Gold difference per minute">
    <line class="zero" x1="0" x2={W} y1={MID} y2={MID} />
    {#each l.minutes as m (m.minute)}
      <rect
        class:up={m.gold_diff >= 0}
        x={x(m.minute) - bar / 2}
        width={bar}
        y={Math.min(MID, y(m.gold_diff))}
        height={Math.max(1, Math.abs(y(m.gold_diff) - MID))}
      ><title>{m.minute}:00 {signed(m.gold_diff, gold)}</title></rect>
      <text x={x(m.minute)} y={H + 12}>{m.minute}</text>
    {/each}
    {#each l.events as e (e.time)}
      <circle class:ours={e.ours} cx={killX(e.time)} cy="6" r="4"><title>{clockText(e.time)} {e.killer != null ? name(e.killer) : 'Executed'} → {name(e.victim)}</title></circle>
    {/each}
  </svg>

  <table>
    <thead>
      <tr><th>Minute</th>{#each l.minutes.filter((m) => m.minute % 2 === 0 || m.minute === 1) as m (m.minute)}<th>{m.minute}</th>{/each}</tr>
    </thead>
    <tbody>
      <tr>
        <th>Level</th>
        {#each l.minutes.filter((m) => m.minute % 2 === 0 || m.minute === 1) as m (m.minute)}
          <td class:good={m.opp_level != null && m.my_level > m.opp_level} class:bad={m.opp_level != null && m.my_level < m.opp_level}>
            {m.my_level}{m.opp_level != null ? `/${m.opp_level}` : ''}
          </td>
        {/each}
      </tr>
      <tr>
        <th>CS</th>
        {#each l.minutes.filter((m) => m.minute % 2 === 0 || m.minute === 1) as m (m.minute)}
          <td class:good={m.cs_ours > m.cs_theirs} class:bad={m.cs_ours < m.cs_theirs}>{m.cs_ours}/{m.cs_theirs}</td>
        {/each}
      </tr>
    </tbody>
  </table>

  {#if l.events.length}
    <h3>Kills in lane</h3>
    <ul>
      {#each l.events as e (e.time)}
        <li class:good={e.ours} class:bad={!e.ours}>
          <span class="num muted">{clockText(e.time)}</span>
          <Icon kind="champ" id={e.victim} size={20} />
          <span>
            {e.killer != null ? name(e.killer) : 'Executed'} killed {name(e.victim)}{e.assists.length ? ` (${e.assists.map(name).join(', ')} assist)` : ''}{e.place ? ` in ${e.place}` : ''}
          </span>
        </li>
      {/each}
    </ul>
  {/if}
  {#if l.backs.length}
    <h3>First backs <span class="muted">(estimated, frames are a minute apart)</span></h3>
    <p class="backs">
      {#each l.backs as b (b.champ)}
        <span class="back" class:ally={b.ally}><Icon kind="champ" id={b.champ} size={20} /> ~{b.minute}:00 · {gold(b.spent)}</span>
      {/each}
    </p>
  {/if}
  {#if rich.length}
    <p class="muted small" class:warn={rich.length >= 2}>You held 1500+ gold unspent at {rich.map((m) => `${m}:00`).join(', ')}.</p>
  {/if}
</section>

<style>
  .verdict {
    margin: 0 0 8px;
    font-size: 17px;
    font-weight: 600;
  }
  svg {
    width: 100%;
    height: auto;
    display: block;
  }
  .zero {
    stroke: var(--line);
  }
  rect {
    fill: var(--threat);
  }
  rect.up {
    fill: var(--good);
  }
  circle {
    fill: var(--threat);
  }
  circle.ours {
    fill: var(--good);
  }
  text {
    font: 11px var(--cond);
    fill: var(--faint);
    text-anchor: middle;
  }
  table {
    width: 100%;
    margin-top: 6px;
    border-collapse: collapse;
    font-size: 14px;
  }
  thead th {
    font: 600 12px var(--cond);
    color: var(--faint);
    text-align: right;
  }
  th:first-child {
    text-align: left;
    font-weight: 400;
    color: var(--muted);
  }
  td {
    text-align: right;
    padding: 1px 0 1px 8px;
    font-variant-numeric: tabular-nums;
  }
  h3 {
    margin: 12px 0 4px;
    font-size: 13px;
    font-weight: 500;
    color: var(--muted);
  }
  ul {
    display: grid;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: 14px;
  }
  li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding-left: 8px;
    border-left: 3px solid var(--threat);
  }
  li.good {
    border-left-color: var(--good);
    color: inherit;
  }
  li.bad {
    color: inherit;
  }
  .backs {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 0;
  }
  .back {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 1px 8px 1px 2px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--threat) 16%, var(--raised));
    font-size: 13px;
  }
  .back.ally {
    background: color-mix(in srgb, var(--good) 16%, var(--raised));
  }
  .small {
    margin: 8px 0 0;
    font-size: 13px;
  }
  .warn {
    color: var(--warn);
  }
</style>
