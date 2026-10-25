<script lang="ts">
  // A row of unit icons with their names underneath; units in `dim` (missing ones) are greyed out.
  import { tft } from '../lib/tft-store.svelte';
  import { tftName } from '../lib/tft';
  import TftIcon from './TftIcon.svelte';

  let { units, dim = [], size = 40 }: { units: string[]; dim?: string[]; size?: number } = $props();
</script>

<ul style:--size="{size}px">
  {#each units as u, i (`${u}-${i}`)}
    {@const off = dim.includes(u)}
    <li class:off>
      <TftIcon kind="unit" api={u} {size} dim={off} />
      <span class="name">{tftName(tft.set, 'unit', u)}</span>
    </li>
  {/each}
</ul>

<style>
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  li {
    width: calc(var(--size) + 12px);
    display: grid;
    justify-items: center;
    gap: 2px;
  }
  .name {
    max-width: 100%;
    font: 500 12px/1.1 var(--cond);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .off .name {
    color: var(--faint);
  }
</style>
