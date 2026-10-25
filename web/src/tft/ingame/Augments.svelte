<script lang="ts">
  // The augments on offer, ranked for the target comp and stage; the top one is tinted.
  import type { AugmentRank } from '../../lib/tft-bindings/AugmentRank';
  import { tft } from '../../lib/tft-store.svelte';
  import { tftName } from '../../lib/tft';
  import TftIcon from '../TftIcon.svelte';

  let { augments }: { augments: AugmentRank[] } = $props();

  const ranked = $derived([...augments].sort((a, b) => b.score - a.score));
</script>

<section class="panel" aria-label="Augment offer">
  <h2>Augment offer, ranked for your comp</h2>
  <ol>
    {#each ranked as a, i (a.augment)}
      <li class:top={i === 0}>
        <span class="pos num">#{i + 1}</span>
        <TftIcon kind="augment" api={a.augment} size={48} />
        <div>
          <div class="name">{tftName(tft.set, 'augment', a.augment)}</div>
          {#if a.reason}<div class="reason">{a.reason}</div>{/if}
        </div>
      </li>
    {/each}
  </ol>
</section>

<style>
  ol {
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 10px;
  }
  li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    border-radius: var(--r);
    background: var(--raised);
  }
  li.top {
    box-shadow: inset 0 0 0 1px var(--gold-dim);
  }
  .pos {
    font-size: 22px;
    font-weight: 700;
    color: var(--muted);
  }
  .top .pos {
    color: var(--gold);
  }
  .name {
    font: 600 21px/1.1 var(--cond);
  }
  .reason {
    font-size: 14px;
    color: var(--muted);
  }
</style>
