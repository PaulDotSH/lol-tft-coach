<script lang="ts">
  // Pre-game: the top comps for the queue with average placement and levelling style.
  // In Choncc or Treat they are ranked by starting carry once the starter is known.
  import type { CompPick } from '../lib/tft-bindings/CompPick';
  import type { TftMode } from '../lib/tft-bindings/TftMode';
  import CompCard from './CompCard.svelte';

  let { meta, mode, queue }: { meta: CompPick[]; mode: TftMode; queue: string } = $props();
</script>

<section class="panel" aria-label="Top comps">
  <h2>
    Top comps · {queue}
    {#if mode === 'choncc'}<span class="choncc">Choncc or Treat</span>{/if}
  </h2>
  {#if mode === 'choncc'}
    <p class="muted note">Start with one 4- or 5-cost unit; loot orbs after each carousel. Comps follow your starter once it's seen.</p>
  {/if}
  {#if meta.length}
    <ol>
      {#each meta as c, i (c.comp)}
        <li>
          <span class="pos num">{i + 1}</span>
          <CompCard pick={c} showFit={false} />
        </li>
      {/each}
    </ol>
  {:else}
    <p class="empty">Loading comps for this queue…</p>
  {/if}
</section>

<style>
  h2 {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .choncc {
    padding: 1px 8px;
    border-radius: 4px;
    background: color-mix(in srgb, #c9a6ff 22%, transparent);
    color: #d9c2ff;
    font: 600 14px var(--cond);
  }
  .note {
    margin: 0 0 10px;
    font-size: 14px;
  }
  ol {
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    gap: 6px;
  }
  li {
    display: grid;
    grid-template-columns: 26px minmax(0, 1fr);
    gap: 10px;
    align-items: start;
    padding: 8px 10px;
    border-radius: var(--r);
    background: var(--raised);
  }
  .pos {
    font-size: 20px;
    font-weight: 700;
    color: var(--muted);
  }
  li:first-child .pos {
    color: var(--gold);
  }
</style>
