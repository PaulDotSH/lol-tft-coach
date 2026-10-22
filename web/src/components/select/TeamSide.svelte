<script lang="ts">
  // One team's composition: damage mix, frontline/engage/peel/CC, gaps, and the
  // factors behind its draft estimate. Shared by the Rift draft panel and the ARAM team panel.
  import type { DraftSide } from '../../lib/bindings/DraftSide';
  import { store } from '../../lib/store.svelte';
  import { champName } from '../../lib/ui';
  import Icon from '../Icon.svelte';
  import { pts, tone } from './draft';

  let { title, side: s, icons = false }: { title: string; side: DraftSide; icons?: boolean } = $props();
  const name = (k: number) => (store.ui ? champName(store.ui, k) : '');
</script>

<div class="side">
  <div class="head">
    <h2>{title}</h2>
    <span class="total num {tone(s.total)}" title="Sum of the factors below, win-rate points vs an even draft"
      >{pts(s.total)}</span
    >
  </div>
  {#if icons && s.champs.length}
    <div class="champs">{#each s.champs as c (c)}<Icon kind="champ" id={c} size={34} />{/each}</div>
  {/if}
  {#if s.champs.length}
    <div class="mix" title="Damage mix (tanks count half)">
      <span class="ad" style:flex-grow={s.physical}></span>
      <span class="ap" style:flex-grow={s.magic}></span>
      <span class="tr" style:flex-grow={s.true_dmg}></span>
    </div>
    <div class="stats muted">
      <span><b class="num">{Math.round(s.physical * 100)}</b> AD</span>
      <span><b class="num">{Math.round(s.magic * 100)}</b> AP</span>
      {#if s.true_dmg >= 0.01}<span><b class="num">{Math.round(s.true_dmg * 100)}</b> true</span>{/if}
      <span class="sep"></span>
      <span><b class="num">{s.frontline}</b> frontline</span>
      <span><b class="num">{s.engage}</b> engage</span>
      <span title="Shields, heals on allies, disengage"><b class="num">{s.peel.toFixed(1)}</b> peel</span>
      <span><b class="num">{s.cc.toFixed(1)}</b> CC</span>
    </div>
  {/if}
  {#if s.gaps.length}
    <div class="gaps">{#each s.gaps as g (g)}<span class="gap">{g}</span>{/each}</div>
  {/if}
  <ul class="factors">
    {#each s.factors as f (f.label)}
      <li>
        <span class="label">{f.label}</span>
        <span class="detail muted">{f.detail}</span>
        <span class="num pts {tone(f.points, 0.1)}">{pts(f.points)}</span>
      </li>
    {:else}
      <li class="empty">{s.champs.length >= 3 ? 'No gaps found.' : 'Waiting for picks or data.'}</li>
    {/each}
  </ul>
  {#if s.duos.length}
    <div class="duos">
      {#each s.duos.slice(0, 3) as d (`${d.a}-${d.b}`)}
        <span class="duo" title="{name(d.a)} + {name(d.b)}: {d.games.toLocaleString()} games together">
          <Icon kind="champ" id={d.a} size={22} /><Icon kind="champ" id={d.b} size={22} />
          <span class="num {tone(d.delta, 0.5)}">{pts(d.delta)}</span>
        </span>
      {/each}
    </div>
  {/if}
</div>

<style>
  .side {
    display: grid;
    gap: 6px;
    align-content: start;
  }
  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
  }
  .head h2 {
    margin: 0;
  }
  .total {
    font-size: 24px;
    font-weight: 700;
  }
  .champs {
    display: flex;
    gap: 6px;
  }
  .mix {
    display: flex;
    height: 6px;
    border-radius: 3px;
    overflow: hidden;
    background: var(--line);
  }
  .ad {
    background: var(--warn);
  }
  .ap {
    background: #7fa6e8;
  }
  .tr {
    background: var(--text);
  }
  .stats {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 10px;
    font-size: 13px;
  }
  .stats b {
    color: var(--text);
    font-size: 15px;
    font-weight: 600;
  }
  .sep {
    flex: 1;
  }
  .gaps {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .gap {
    padding: 0 6px;
    border-radius: 4px;
    font-size: 12px;
    color: var(--warn);
    box-shadow: inset 0 0 0 1px var(--gold-dim);
  }
  .factors {
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    gap: 2px;
  }
  .factors li {
    display: grid;
    grid-template-columns: 88px minmax(0, 1fr) auto;
    align-items: baseline;
    gap: 8px;
  }
  .factors li.empty {
    display: block;
    font-size: 13px;
  }
  .label {
    font: 600 15px/1.3 var(--cond);
  }
  .detail {
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pts {
    font-size: 16px;
    font-weight: 600;
  }
  .duos {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 12px;
  }
  .duo {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    font-size: 14px;
  }
  .duo .num {
    margin-left: 4px;
  }
</style>
