<script lang="ts">
  // One comp pick: name, how much of it I have (fit), its average placement, levelling style,
  // and its units (owned lit, missing greyed). `big` is the in-game target; `compact` a list row.
  import type { CompPick } from '../lib/tft-bindings/CompPick';
  import { avgText, share } from '../lib/tft';
  import Units from './Units.svelte';

  let {
    pick,
    big = false,
    showFit = true,
  }: { pick: CompPick; big?: boolean; showFit?: boolean } = $props();

  const units = $derived([...pick.have, ...pick.missing]);
</script>

<div class="comp" class:big>
  <div class="head">
    <span class="name">{pick.name}</span>
    {#if pick.levelling}<span class="chip">{pick.levelling}</span>{/if}
    <span class="avg num" title="Average placement">avg {avgText(pick.avg_place)}</span>
  </div>
  {#if showFit}
    <div class="fit" title="How much of the comp I already have">
      <span class="bar"><span style:width={share(pick.fit)}></span></span>
      <span class="num">{share(pick.fit)}</span>
      <span class="muted small">fit · {pick.have.length} have, {pick.missing.length} missing</span>
    </div>
  {/if}
  <Units {units} dim={showFit ? pick.missing : []} size={big ? 44 : 32} />
</div>

<style>
  .comp {
    display: grid;
    gap: 6px;
    min-width: 0;
  }
  .head {
    display: flex;
    align-items: baseline;
    gap: 10px;
    min-width: 0;
  }
  .name {
    min-width: 0;
    font: 600 19px/1.1 var(--cond);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .big .name {
    font: 700 32px/1.05 var(--cond);
    color: var(--gold);
  }
  .chip {
    flex: none;
    padding: 0 6px;
    border: 1px solid var(--line);
    border-radius: 4px;
    font: 600 13px/1.4 var(--cond);
    color: var(--muted);
  }
  .avg {
    flex: none;
    margin-left: auto;
    font-size: 16px;
    color: var(--muted);
  }
  .big .avg {
    font-size: 20px;
  }
  .fit {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 15px;
  }
  .bar {
    width: 120px;
    height: 6px;
    border-radius: 3px;
    background: var(--line);
    overflow: hidden;
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--good);
  }
  .small {
    font-size: 13px;
  }
</style>
