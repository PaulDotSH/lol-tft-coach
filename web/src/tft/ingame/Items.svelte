<script lang="ts">
  // Items to build from the components I hold: the item, its two components, who should
  // hold it (from the target comp's carry builds) and whether to slam it now.
  import type { ItemAdvice } from '../../lib/tft-bindings/ItemAdvice';
  import { tft } from '../../lib/tft-store.svelte';
  import { tftName } from '../../lib/tft';
  import TftIcon from '../TftIcon.svelte';

  let { items }: { items: ItemAdvice[] } = $props();
</script>

<section class="panel" aria-label="Items">
  <h2>Items</h2>
  {#if items.length}
    <ul>
      {#each items as it, i (`${it.item}-${i}`)}
        <li class:now={it.now}>
          <TftIcon kind="item" api={it.item} size={48} />
          <div class="body">
            <div class="top">
              <span class="name">{tftName(tft.set, 'item', it.item)}</span>
              {#if it.now}<span class="slam">Slam now</span>{/if}
            </div>
            <div class="recipe">
              {#each it.from as c, j (`${c}-${j}`)}
                {#if j}<span class="plus" aria-hidden="true">+</span>{/if}
                <TftIcon kind="item" api={c} size={22} />
              {/each}
              {#if it.holder}
                <span class="arrow" aria-hidden="true">→</span>
                <TftIcon kind="unit" api={it.holder} size={24} />
                <span class="holder">{tftName(tft.set, 'unit', it.holder)}</span>
              {/if}
            </div>
            {#if it.reason}<p class="reason">{it.reason}</p>{/if}
          </div>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="empty">No components to combine yet.</p>
  {/if}
</section>

<style>
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    gap: 8px;
  }
  li {
    display: flex;
    gap: 12px;
    align-items: flex-start;
    padding: 8px 10px;
    border-radius: var(--r);
    background: var(--raised);
  }
  li.now {
    box-shadow: inset 0 0 0 1px var(--gold-dim);
  }
  .body {
    min-width: 0;
    display: grid;
    gap: 4px;
  }
  .top {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  .name {
    font: 600 20px/1.1 var(--cond);
  }
  .slam {
    padding: 0 8px;
    border-radius: 4px;
    background: var(--gold);
    color: var(--bg);
    font: 700 15px/1.4 var(--cond);
    text-transform: uppercase;
  }
  .recipe {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 14px;
  }
  .plus,
  .arrow {
    color: var(--faint);
  }
  .arrow {
    margin: 0 2px 0 4px;
  }
  .holder {
    font: 600 15px var(--cond);
  }
  .reason {
    margin: 0;
    font-size: 14px;
    color: var(--muted);
  }
</style>
