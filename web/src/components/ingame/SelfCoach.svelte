<script lang="ts">
  import type { SelfStats } from '../../lib/bindings/SelfStats';
  import { store } from '../../lib/store.svelte';
  import { champName, gold, signed } from '../../lib/ui';
  import Icon from '../Icon.svelte';

  let { me, laneOpp }: { me: SelfStats; laneOpp: number | null } = $props();
  const oppName = $derived(laneOpp != null && store.ui ? champName(store.ui, laneOpp) : 'lane opponent');
  const tone = (n: number | null) => (n == null || n === 0 ? '' : n > 0 ? 'good' : 'bad');
</script>

<section class="panel row" aria-label="You">
  <div class="stat">
    <h2>CS per minute</h2>
    <div class="big num">
      <span class:bad={me.cs_benchmark != null && me.cs_per_min < me.cs_benchmark}>{me.cs_per_min.toFixed(1)}</span>
      {#if me.cs_benchmark != null}<small class="muted">/ {me.cs_benchmark.toFixed(1)} avg</small>{/if}
    </div>
    {#if me.cs_diff != null}
      <div class="sub {tone(me.cs_diff)}">{signed(me.cs_diff)} CS vs average now</div>
    {/if}
  </div>

  {#if me.item_gold_vs_opp != null || me.level_vs_opp != null}
    <div class="stat">
      <h2>Vs {oppName}</h2>
      <div class="big num">
        {#if me.item_gold_vs_opp != null}
          <span class={tone(me.item_gold_vs_opp)}>{signed(me.item_gold_vs_opp, gold)}</span><small class="muted"> gold</small>
        {/if}
        {#if me.level_vs_opp != null}
          <span class={tone(me.level_vs_opp)}>{signed(me.level_vs_opp)}</span><small class="muted"> lvl</small>
        {/if}
      </div>
    </div>
  {/if}

  <div class="stat">
    <h2>Deaths</h2>
    <div class="big num">
      {me.deaths}
      {#each me.killers as [champ, n] (champ)}
        <span class="killer"><Icon kind="champ" id={champ} size={26} /><small>{n}×</small></span>
      {/each}
    </div>
  </div>

  {#if me.vision > 0 || me.vision_benchmark != null}
  <div class="stat">
    <h2>Vision score</h2>
    <div class="big num">
      <span class:bad={me.vision_benchmark != null && me.vision < me.vision_benchmark}>{me.vision.toFixed(1)}</span>
      {#if me.vision_benchmark != null}<small class="muted">/ {me.vision_benchmark.toFixed(1)} avg</small>{/if}
    </div>
  </div>
  {/if}
</section>

<style>
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 10px 36px;
  }
  .stat h2 {
    margin-bottom: 2px;
  }
  .big {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 30px;
    font-weight: 600;
    line-height: 1.1;
  }
  small {
    font-size: 17px;
    font-weight: 500;
  }
  .sub {
    font-size: 14px;
  }
  .killer {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    margin-left: 6px;
  }
  .killer small {
    color: var(--threat);
  }
</style>
