<script lang="ts">
  import type { Recap } from '../../lib/bindings/Recap';
  import { store } from '../../lib/store.svelte';
  import { champName, clockText, MODE_NAMES } from '../../lib/ui';
  import Icon from '../Icon.svelte';

  let { recap }: { recap: Recap } = $props();
  const s = $derived(recap.stats);
  const b = $derived(recap.benchmarks);
  const min = $derived(Math.max(1, s.duration_s / 60));

  // [label, yours per minute, benchmark per minute, digits]
  const rows = $derived<[string, number, number | null, number][]>([
    ['CS / min', s.cs / min, b.cs_per_min, 1],
    ['Gold / min', s.gold / min, b.gold_per_min, 0],
    ['Damage / min', s.damage_to_champs / min, b.damage_per_min, 0],
    ['Vision / min', s.vision / min, b.vision_per_min, 2],
  ]);
  const final = $derived(new Set(s.items));
</script>

<section class="panel">
  <h2>Last game</h2>
  <div class="head">
    <Icon kind="champ" id={s.champ} size={48} />
    <div>
      <div class="title">
        <span class={s.win ? 'good' : 'bad'}>{s.win ? 'Victory' : 'Defeat'}</span>
        {store.ui ? champName(store.ui, s.champ) : ''}
      </div>
      <div class="muted">{MODE_NAMES[s.mode]}, {clockText(s.duration_s)}</div>
    </div>
    <div class="kda num">{s.kills}/{s.deaths}/{s.assists}</div>
  </div>

  <table>
    <tbody>
      {#each rows as [label, mine, avg, d] (label)}
        <tr>
          <th>{label}</th>
          <td class="num" class:good={avg != null && mine >= avg} class:bad={avg != null && mine < avg * 0.9}>{mine.toFixed(d)}</td>
          <td class="num muted">{avg != null ? `avg ${avg.toFixed(d)}` : ''}</td>
        </tr>
      {/each}
    </tbody>
  </table>

  <div class="builds">
    <span class="muted">Built</span>
    <div class="items">{#each s.items as id, i (`${i}-${id}`)}<Icon kind="item" {id} size={30} />{/each}</div>
    <span class="muted">Coach</span>
    <div class="items">
      {#each recap.recommended as id, i (`${i}-${id}`)}
        <span class:missed={!final.has(id)}><Icon kind="item" {id} size={30} /></span>
      {/each}
    </div>
  </div>

  {#if recap.notes.length}
    <ul class="notes">
      {#each recap.notes as n, i (i)}<li>{n}</li>{/each}
    </ul>
  {/if}
</section>

<style>
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .title {
    font: 600 21px/1.1 var(--cond);
  }
  .kda {
    margin-left: auto;
    font-size: 26px;
    font-weight: 600;
  }
  table {
    width: 100%;
    margin: 12px 0;
    border-collapse: collapse;
  }
  th {
    text-align: left;
    font-weight: 400;
    color: var(--muted);
  }
  td {
    text-align: right;
    font-size: 18px;
    padding: 1px 0 1px 12px;
  }
  .builds {
    display: grid;
    grid-template-columns: auto 1fr;
    align-items: center;
    gap: 6px 12px;
  }
  .items {
    display: flex;
    gap: 4px;
  }
  .missed {
    display: inline-flex;
    border-radius: 4px;
    box-shadow: 0 0 0 2px var(--warn);
  }
  .notes {
    margin: 12px 0 0;
    padding-left: 18px;
    font-size: 14px;
    color: var(--muted);
  }
  .notes li + li {
    margin-top: 4px;
  }
</style>
