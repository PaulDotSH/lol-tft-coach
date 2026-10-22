<script lang="ts">
  // Buy next ETA from tracked income, plus the next gold amounts that buy more.
  import type { GoldEta } from '../../lib/bindings/GoldEta';
  import Icon from '../Icon.svelte';

  let { eta }: { eta: GoldEta } = $props();
  const secs = (s: number) => (s >= 90 ? `~${Math.round(s / 60)} min` : `~${Math.max(5, Math.round(s / 5) * 5)} s`);
</script>

<div class="eta">
  <div class="line">
    {#if eta.missing === 0}
      <strong class="good">Affordable now</strong>
    {:else}
      <span><span class="num">{eta.missing.toLocaleString()}</span> g short</span>
      {#if eta.eta_s != null}<strong class="num">{secs(eta.eta_s)}</strong>{/if}
    {/if}
    {#if eta.per_min != null}<span class="muted num" title="Income over the last 2 min, purchases left out">{Math.round(eta.per_min)} g/min</span>{/if}
  </div>
  {#each eta.backs as b (b.gold)}
    <div class="back">
      <span class="muted">at</span>
      <span class="num gold">{b.gold.toLocaleString()} g</span>
      {#if b.in_s != null}<span class="muted num">({secs(b.in_s)})</span>{/if}
      <span class="icons">{#each b.items as id, i (i)}<Icon kind="item" id={id} size={22} />{/each}</span>
    </div>
  {/each}
</div>

<style>
  .eta {
    display: grid;
    gap: 3px;
    margin-top: 10px;
    font-size: 16px;
  }
  .line {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0 10px;
  }
  .back {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 15px;
  }
  .icons {
    display: inline-flex;
    gap: 3px;
  }
  .gold {
    color: var(--gold);
  }
</style>
