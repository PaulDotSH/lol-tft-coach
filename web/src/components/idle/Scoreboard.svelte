<script lang="ts">
  // Both teams' end-of-game lines; my row highlighted, damage as a bar.
  import type { MatchPlayer } from '../../lib/bindings/MatchPlayer';
  import { gold, LANE_NAMES } from '../../lib/ui';
  import Icon from '../Icon.svelte';

  let { players, win }: { players: MatchPlayer[]; win: boolean } = $props();
  const top = $derived(Math.max(1, ...players.map((p) => p.damage)));
  const teams = $derived([
    { label: 'Your team', won: win, rows: players.filter((p) => p.ally) },
    { label: 'Enemy team', won: !win, rows: players.filter((p) => !p.ally) },
  ]);
</script>

{#each teams as t (t.label)}
  <table>
    <thead>
      <tr>
        <th class="team" colspan="2"><span class={t.won ? 'good' : 'bad'}>{t.won ? 'Victory' : 'Defeat'}</span> · {t.label}</th>
        <th>KDA</th>
        <th>CS</th>
        <th>Gold</th>
        <th>Damage</th>
        <th>Vision</th>
        <th>Items</th>
      </tr>
    </thead>
    <tbody>
      {#each t.rows as p, i (i)}
        <tr class:me={p.me}>
          <td class="ic"><Icon kind="champ" id={p.champ} size={28} /></td>
          <td class="who">
            <span class="name">{p.me ? 'You' : p.name || '—'}</span>
            <span class="muted lvl">{p.lane ? LANE_NAMES[p.lane] : ''} · {p.level}</span>
          </td>
          <td class="num">{p.kills}/{p.deaths}/{p.assists}</td>
          <td class="num">{p.cs}</td>
          <td class="num">{gold(p.gold)}</td>
          <td class="dmg">
            <span class="num">{gold(p.damage)}</span>
            <span class="bar"><i style:width="{(100 * p.damage) / top}%"></i></span>
          </td>
          <td class="num">{p.vision}</td>
          <td><div class="items">{#each p.items as id, j (`${j}-${id}`)}<Icon kind="item" {id} size={22} />{/each}</div></td>
        </tr>
      {/each}
    </tbody>
  </table>
{/each}

<style>
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 14px;
  }
  table + table {
    margin-top: 10px;
  }
  th {
    text-align: left;
    font: 600 12px var(--cond);
    color: var(--faint);
    padding: 0 6px 4px 0;
    white-space: nowrap;
  }
  th.team {
    font-size: 13px;
    color: var(--muted);
  }
  td {
    padding: 2px 6px 2px 0;
    vertical-align: middle;
    white-space: nowrap;
  }
  tr.me td {
    background: color-mix(in srgb, var(--gold) 12%, transparent);
  }
  tr.me .name {
    color: var(--gold);
  }
  .ic {
    width: 30px;
    padding-left: 2px;
  }
  .who {
    display: grid;
    max-width: 130px;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    font-weight: 600;
  }
  .lvl {
    font-size: 12px;
  }
  .dmg {
    min-width: 76px;
  }
  .bar {
    display: block;
    height: 3px;
    margin-top: 2px;
    background: var(--raised);
    border-radius: 2px;
  }
  .bar i {
    display: block;
    height: 100%;
    background: var(--threat);
    border-radius: 2px;
  }
  .items {
    display: flex;
    gap: 2px;
  }
</style>
