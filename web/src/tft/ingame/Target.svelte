<script lang="ts">
  // The automatic target comp (best fit for what I own) and the next two alternatives.
  import type { CompPick } from '../../lib/tft-bindings/CompPick';
  import CompCard from '../CompCard.svelte';

  let { comps }: { comps: CompPick[] } = $props();

  const target = $derived(comps[0] ?? null);
  const alts = $derived(comps.slice(1, 3));
</script>

<section class="panel target" aria-label="Target comp">
  <h2>Target comp</h2>
  {#if target}
    <CompCard pick={target} big />
    {#if alts.length}
      <h3>Alternatives</h3>
      <div class="alts">
        {#each alts as c (c.comp)}<CompCard pick={c} />{/each}
      </div>
    {/if}
  {:else}
    <p class="empty">Picking a comp once a few units are on the board…</p>
  {/if}
</section>

<style>
  .target {
    border: 1px solid var(--gold-dim);
  }
  h3 {
    margin: 14px 0 6px;
    padding-top: 10px;
    border-top: 1px solid var(--line);
    font: 600 14px var(--cond);
    color: var(--muted);
  }
  .alts {
    display: grid;
    gap: 12px;
  }
</style>
