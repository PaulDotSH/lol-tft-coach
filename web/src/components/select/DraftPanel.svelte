<script lang="ts">
  // Both teams' draft as a breakdown: lane matchups, damage mix, frontline/engage/CC,
  // gaps and the factors behind each side's estimate. Shows options, never a verdict.
  import type { DraftReport } from '../../lib/bindings/DraftReport';
  import type { DraftSide } from '../../lib/bindings/DraftSide';
  import { store } from '../../lib/store.svelte';
  import { champName, LANE_NAMES, pct } from '../../lib/ui';
  import Icon from '../Icon.svelte';

  let { draft }: { draft: DraftReport } = $props();

  const pts = (n: number) => (n > 0 ? `+${n.toFixed(1)}` : n < 0 ? `−${(-n).toFixed(1)}` : '±0.0');
  const tone = (n: number, eps = 0.25) => (n > eps ? 'good' : n < -eps ? 'bad' : 'muted');
  const name = (k: number) => (store.ui ? champName(store.ui, k) : '');
</script>

{#snippet side(title: string, s: DraftSide)}
  <div class="side">
    <div class="head">
      <h2>{title}</h2>
      <span class="total num {tone(s.total)}" title="Sum of the factors below, win-rate points vs an even draft"
        >{pts(s.total)}</span
      >
    </div>
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
        <li class="empty">Waiting for picks or data.</li>
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
{/snippet}

<section class="panel draft">
  {#if draft.lanes.length}
    <ol class="lanes">
      {#each draft.lanes as l (l.lane)}
        <li title={l.ally_wr != null ? `${l.games.toLocaleString()} games` : ''}>
          <span class="lane muted">{LANE_NAMES[l.lane]}</span>
          <span class="pair">
            {#if l.ally != null}<Icon kind="champ" id={l.ally} size={30} />{:else}<span class="slot"></span>{/if}
            {#if l.ally_wr != null}
              <span class="num wr {tone(l.ally_wr - 50, 2.5)}">{pct(l.ally_wr)}</span>
            {:else}
              <span class="vs muted">vs</span>
            {/if}
            {#if l.enemy != null}
              <span class="enemy" title="Likely {LANE_NAMES[l.lane]}: {name(l.enemy)}"><Icon kind="champ" id={l.enemy} size={30} /></span>
            {:else}<span class="slot"></span>{/if}
          </span>
        </li>
      {/each}
    </ol>
  {/if}
  <div class="sides">
    {@render side('Your team', draft.allies)}
    {@render side('Enemy team', draft.enemies)}
  </div>
  <p class="note">Win-rate points vs an even draft, from lolalytics. Enemy lanes are a guess from play rates.</p>
</section>

<style>
  .draft {
    display: grid;
    gap: 12px;
  }
  .lanes {
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(118px, 1fr));
    gap: 8px;
  }
  .lanes li {
    display: grid;
    justify-items: center;
    gap: 2px;
    padding: 4px 0 6px;
    border-radius: var(--r);
    background: var(--raised);
  }
  .lane {
    font: 600 12px/1.2 var(--cond);
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
  .pair {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .wr {
    min-width: 46px;
    text-align: center;
    font-size: 17px;
    font-weight: 600;
  }
  .vs {
    min-width: 46px;
    text-align: center;
    font-size: 12px;
  }
  .enemy :global(.icon) {
    box-shadow: 0 0 0 2px var(--threat);
  }
  .slot {
    width: 30px;
    height: 30px;
    border-radius: 50%;
    border: 1px dashed var(--line);
  }
  .sides {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 20px;
  }
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
  .note {
    margin: 0;
    font-size: 12px;
    color: var(--faint);
  }
  @media (max-width: 760px) {
    .sides {
      grid-template-columns: 1fr;
    }
  }
</style>
