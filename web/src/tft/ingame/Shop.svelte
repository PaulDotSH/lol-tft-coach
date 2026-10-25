<script lang="ts">
  // The shop as five slots in game order: buy (lit) or skip (dimmed), why, and how many
  // other boards hold the unit. Empty or unread slots stay as blank frames.
  import type { ShopAdvice } from '../../lib/tft-bindings/ShopAdvice';
  import { tft } from '../../lib/tft-store.svelte';
  import { costColor, tftName, unitCost } from '../../lib/tft';
  import TftIcon from '../TftIcon.svelte';

  let { shop }: { shop: ShopAdvice[] } = $props();

  const slots = $derived([0, 1, 2, 3, 4].map((i) => shop.find((s) => s.slot === i) ?? null));
</script>

<section class="panel" aria-label="Shop">
  <h2>Shop</h2>
  <ol>
    {#each slots as s, i (i)}
      {#if s}
        <li class:buy={s.buy} style:--cost={costColor(unitCost(tft.set, s.unit))}>
          <TftIcon kind="unit" api={s.unit} size={52} dim={!s.buy} />
          <div class="body">
            <div class="top">
              <span class="name">{tftName(tft.set, 'unit', s.unit)}</span>
              <span class="cost num">{unitCost(tft.set, s.unit) || ''}</span>
            </div>
            <span class="verdict">{s.buy ? 'Buy' : 'Skip'}</span>
            {#if s.contested}
              <span class="contested" title="Held on {s.contested} other board{s.contested > 1 ? 's' : ''}">
                ×{s.contested} board{s.contested > 1 ? 's' : ''}
              </span>
            {/if}
            {#if s.reason}<p class="reason">{s.reason}</p>{/if}
          </div>
        </li>
      {:else}
        <li class="blank" aria-label="Empty slot"></li>
      {/if}
    {/each}
  </ol>
</section>

<style>
  ol {
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 8px;
  }
  li {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    min-height: 84px;
    padding: 8px 10px;
    border-radius: var(--r);
    border-top: 3px solid var(--cost);
    background: var(--raised);
    opacity: 0.7;
  }
  li.buy {
    opacity: 1;
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--good) 70%, transparent);
    background: color-mix(in srgb, var(--good) 10%, var(--raised));
  }
  li.blank {
    border: 1px dashed var(--line);
    background: none;
  }
  .body {
    min-width: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 2px 8px;
  }
  .top {
    flex-basis: 100%;
    display: flex;
    align-items: baseline;
    gap: 6px;
    min-width: 0;
  }
  .name {
    font: 600 19px/1.1 var(--cond);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cost {
    color: var(--cost);
    font-size: 15px;
  }
  .verdict {
    font: 700 16px var(--cond);
    text-transform: uppercase;
    color: var(--faint);
  }
  .buy .verdict {
    color: var(--good);
  }
  .contested {
    padding: 0 6px;
    border-radius: 4px;
    background: color-mix(in srgb, var(--threat) 25%, transparent);
    color: var(--threat);
    font: 600 14px var(--cond);
  }
  .reason {
    flex-basis: 100%;
    margin: 2px 0 0;
    font-size: 13px;
    line-height: 1.25;
    color: var(--muted);
  }
  .buy .reason {
    color: var(--text);
  }
  @media (max-width: 760px) {
    ol {
      grid-template-columns: 1fr;
    }
  }
</style>
