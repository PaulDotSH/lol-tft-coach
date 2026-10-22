<script lang="ts">
  // Champion / item / augment icon with a tooltip name. Renders a blank tile until static data loads.
  import { store } from '../lib/store.svelte';
  import { augmentIcon, champIcon, champName, itemIcon, itemName } from '../lib/ui';

  let { kind, id, size = 32, title }: { kind: 'champ' | 'item' | 'augment'; id: number; size?: number; title?: string } =
    $props();

  const src = $derived.by(() => {
    const ui = store.ui;
    if (!ui) return '';
    return kind === 'champ' ? champIcon(ui, id) : kind === 'item' ? itemIcon(ui, id) : augmentIcon(ui, id);
  });
  const name = $derived.by(() => {
    const ui = store.ui;
    if (title !== undefined || !ui) return title ?? '';
    return kind === 'champ' ? champName(ui, id) : kind === 'item' ? itemName(ui, id) : (ui.augments[id] ?? '');
  });
</script>

{#if src}
  <img class="icon {kind}" {src} alt={name} title={name} width={size} height={size} decoding="async" />
{:else}
  <span class="icon {kind}" style:width="{size}px" style:height="{size}px" title={name}></span>
{/if}

<style>
  .icon {
    display: inline-block;
    flex: none;
    border-radius: 4px;
    background: var(--raised);
    vertical-align: middle;
    object-fit: cover;
  }
  .champ {
    border-radius: 50%;
  }
  .item {
    box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.5);
  }
  .augment {
    background: none;
  }
</style>
