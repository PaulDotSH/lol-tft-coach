<script lang="ts">
  import type { Threat } from '../../lib/bindings/Threat';
  import { gold, signed } from '../../lib/ui';
  import Icon from '../Icon.svelte';

  let { threats }: { threats: Threat[] } = $props();
  const sorted = $derived([...threats].sort((a, b) => b.score - a.score));
</script>

<section class="panel">
  <h2>Threats</h2>
  {#if sorted.length === 0}
    <p class="empty">No enemy data yet.</p>
  {/if}
  <ul>
    {#each sorted as t (t.champ)}
      <li class:opp={t.lane_opp} style:--s="{t.score}%" style:--a={0.25 + t.score / 140}>
        <Icon kind="champ" id={t.champ} size={44} title={t.name} />
        <div class="body">
          <div class="top">
            <span class="name">{t.name}</span>
            {#if t.lane_opp}<span class="lane">Lane</span>{/if}
            <span class="chip dmg">{t.damage}</span>
            {#each t.flags as f (f)}<span class="chip">{f}</span>{/each}
          </div>
          <div class="stats num">
            <span title="Kills / deaths / assists">{t.kills}/{t.deaths}/{t.assists}</span>
            <span title="Item gold vs lobby average" class:bad={t.item_gold_diff > 500}>{signed(t.item_gold_diff, gold)} g</span>
            <span title="Level vs lobby average">{signed(t.level_diff)} lvl</span>
            {#if t.kills_on_me > 0}<span class="bad">killed you {t.kills_on_me}×</span>{/if}
          </div>
          <div class="bar"><span></span></div>
        </div>
        <div class="score num">{Math.round(t.score)}</div>
      </li>
    {/each}
  </ul>
</section>

<style>
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    gap: 10px;
  }
  li {
    display: grid;
    grid-template-columns: 44px minmax(0, 1fr) 40px;
    align-items: center;
    column-gap: 10px;
  }
  .opp :global(.icon) {
    box-shadow: 0 0 0 2px var(--gold);
  }
  .top {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 6px;
  }
  .name {
    font: 600 19px/1.1 var(--cond);
  }
  .lane {
    font: 600 13px var(--cond);
    color: var(--gold);
  }
  .chip {
    padding: 0 6px;
    border-radius: 3px;
    background: var(--raised);
    font-size: 12px;
    line-height: 18px;
    color: var(--muted);
  }
  .dmg {
    color: var(--text);
  }
  .stats {
    display: flex;
    flex-wrap: wrap;
    gap: 0 12px;
    font-size: 16px;
    color: var(--muted);
  }
  .bar {
    margin-top: 3px;
    height: 5px;
    border-radius: 3px;
    background: var(--raised);
    overflow: hidden;
  }
  .bar span {
    display: block;
    height: 100%;
    width: var(--s);
    background: rgb(228 104 93 / var(--a));
    transition: width 0.4s ease-out;
  }
  .score {
    font-size: 26px;
    font-weight: 700;
    text-align: right;
  }
</style>
