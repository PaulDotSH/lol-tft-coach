<script lang="ts">
  // Choncc or Treat additions: the 4/5-cost starter, comps built around it, which loot to
  // use first, and mode notes (team size from Crown/Chalice, comeback orb).
  import type { ChonccAdvice } from '../../lib/tft-bindings/ChonccAdvice';
  import { tft } from '../../lib/tft-store.svelte';
  import { tftName } from '../../lib/tft';
  import CompCard from '../CompCard.svelte';
  import TftIcon from '../TftIcon.svelte';

  let { choncc }: { choncc: ChonccAdvice } = $props();
</script>

<section class="panel choncc" aria-label="Choncc or Treat">
  <h2>Choncc or Treat</h2>
  <div class="starter">
    {#if choncc.starter}
      <TftIcon kind="unit" api={choncc.starter} size={56} />
      <div>
        <span class="muted small">Starter</span>
        <div class="sname">{tftName(tft.set, 'unit', choncc.starter)}</div>
      </div>
    {:else}
      <p class="empty">Starter not seen yet.</p>
    {/if}
  </div>

  {#if choncc.comps.length}
    <h3>Comps for {choncc.starter ? tftName(tft.set, 'unit', choncc.starter) : 'the starter'}</h3>
    <div class="comps">
      {#each choncc.comps as c (c.comp)}<CompCard pick={c} />{/each}
    </div>
  {/if}

  {#if choncc.loot.length}
    <h3>Loot first</h3>
    <ol class="loot">
      {#each choncc.loot as l, i (i)}<li>{l}</li>{/each}
    </ol>
  {/if}

  {#if choncc.notes.length}
    <ul class="notes">
      {#each choncc.notes as n, i (i)}<li>{n}</li>{/each}
    </ul>
  {/if}
</section>

<style>
  .choncc {
    border-left: 3px solid #c9a6ff;
  }
  .starter {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .sname {
    font: 700 26px/1.05 var(--cond);
  }
  .small {
    font-size: 13px;
  }
  h3 {
    margin: 14px 0 6px;
    font: 600 14px var(--cond);
    color: var(--muted);
  }
  .comps {
    display: grid;
    gap: 12px;
  }
  .loot {
    margin: 0;
    padding-left: 22px;
    display: grid;
    gap: 3px;
    font-size: 16px;
  }
  .notes {
    margin: 12px 0 0;
    padding-left: 18px;
    display: grid;
    gap: 3px;
    font-size: 14px;
    color: var(--muted);
  }
</style>
