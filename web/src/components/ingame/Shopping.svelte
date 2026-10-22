<script lang="ts">
  // What current gold buys right now. Always the same strip (no layout jumps); lit up while dead,
  // which is when ARAM players can shop and anyone has time to look.
  import type { ShoppingList } from '../../lib/bindings/ShoppingList';
  import { clock, store } from '../../lib/store.svelte';
  import { itemName } from '../../lib/ui';
  import Icon from '../Icon.svelte';

  let { list, at }: { list: ShoppingList; at: number } = $props();
  const left = $derived(Math.max(0, Math.ceil(list.respawn_in - (clock.time - at))));
  const title = (item: number, toward: number | null) => {
    const ui = store.ui;
    if (!ui) return '';
    return toward != null ? `${itemName(ui, item)} → ${itemName(ui, toward)}` : itemName(ui, item);
  };
</script>

<section class="shop" class:dead={list.dead} aria-label="Shopping list">
  <div class="status">
    {#if list.dead}
      <span class="label">Back in</span>
      <span class="count num">{left}<small>s</small></span>
    {:else}
      <span class="label">With your gold</span>
    {/if}
    <span class="gold num">{list.gold.toLocaleString()} g</span>
  </div>
  <ul class="buys">
    {#each list.buys as b, i (i)}
      <li class={b.kind} title={title(b.item, b.toward)}>
        <Icon kind="item" id={b.item} size={list.dead ? 48 : 36} title={title(b.item, b.toward)} />
        <span class="price num">{b.price.toLocaleString()}</span>
      </li>
    {:else}
      <li class="none">Nothing affordable yet</li>
    {/each}
  </ul>
  <div class="left num" title="Gold left after these buys">{list.leftover.toLocaleString()} g left</div>
</section>

<style>
  .shop {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 14px;
    min-height: 86px;
    padding: 8px 14px;
    border-radius: 10px;
    border: 1px solid var(--line);
    background: var(--panel);
    transition:
      border-color 0.3s,
      background 0.3s;
  }
  .shop.dead {
    border-color: var(--gold);
    background: linear-gradient(90deg, rgb(220 183 92 / 0.16), var(--panel) 60%);
    box-shadow: 0 0 0 1px rgb(220 183 92 / 0.35);
  }
  .status {
    display: grid;
    justify-items: start;
    min-width: 92px;
  }
  .label {
    font: 600 14px var(--cond);
    color: var(--muted);
  }
  .dead .label {
    color: var(--gold);
  }
  .count {
    font-size: 40px;
    font-weight: 700;
    line-height: 1;
  }
  .count small {
    font-size: 18px;
    color: var(--muted);
  }
  .gold {
    font-size: 17px;
    color: var(--gold);
  }
  .buys {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;
    min-width: 0;
  }
  .buys li {
    display: grid;
    justify-items: center;
    gap: 1px;
  }
  .component :global(.icon) {
    opacity: 0.85;
  }
  .full :global(.icon) {
    box-shadow: 0 0 0 2px var(--gold);
  }
  .price {
    font-size: 14px;
    color: var(--muted);
  }
  .dead .price {
    font-size: 16px;
    color: var(--text);
  }
  .none {
    color: var(--faint);
    font-style: italic;
  }
  .left {
    font-size: 16px;
    color: var(--muted);
    white-space: nowrap;
  }
</style>
