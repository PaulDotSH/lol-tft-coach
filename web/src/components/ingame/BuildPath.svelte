<script lang="ts">
  import type { ItemPick } from '../../lib/bindings/ItemPick';
  import Icon from '../Icon.svelte';

  let { owned, path, source }: { owned: number[]; path: ItemPick[]; source: string } = $props();
</script>

<section class="panel">
  <h2>Full build <span class="src">{source}</span></h2>
  <ol>
    {#each owned as id, i (`o${i}-${id}`)}
      <li class="owned"><Icon kind="item" {id} size={40} /></li>
    {/each}
    {#each path as p, i (`p${i}-${p.item}`)}
      <li class:next={i === 0}><Icon kind="item" id={p.item} size={40} title="{p.name} ({p.cost} g)" /></li>
    {/each}
  </ol>
</section>

<style>
  .src {
    margin-left: 6px;
    font: 500 12px/1 var(--sans);
    color: var(--faint);
  }
  ol {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
  }
  li {
    display: flex;
    position: relative;
  }
  .owned {
    opacity: 0.45;
  }
  .owned::after {
    content: '✓';
    position: absolute;
    right: -3px;
    bottom: -5px;
    font-size: 13px;
    color: var(--good);
  }
  .next :global(.icon) {
    box-shadow: 0 0 0 2px var(--gold);
  }
  li + li:not(.owned)::before {
    content: '';
    position: absolute;
    left: -8px;
    top: 50%;
    width: 8px;
    border-top: 1px solid var(--line);
  }
</style>
