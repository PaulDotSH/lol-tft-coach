<script lang="ts">
  // Unit / item / augment / trait icon from Community Dragon, with the display name as tooltip.
  // Units get a cost-coloured frame; a missing or broken image falls back to a lettered tile.
  import { tft } from '../lib/tft-store.svelte';
  import { costColor, tftIcon, tftName, unitCost, type TftKind } from '../lib/tft';

  let {
    kind,
    api,
    size = 32,
    title,
    dim = false,
  }: { kind: TftKind; api: string; size?: number; title?: string; dim?: boolean } = $props();

  const src = $derived(tftIcon(tft.set, kind, api));
  const name = $derived(title ?? tftName(tft.set, kind, api));
  const frame = $derived(kind === 'unit' ? costColor(unitCost(tft.set, api)) : null);
  const tier = $derived(kind === 'augment' ? (tft.set?.augments[api]?.tier ?? null) : null);
  /** The src that failed to load; a new src gets a fresh try. */
  let failed = $state('');
  const initials = $derived(
    name
      .split(/\s+/)
      .slice(0, 2)
      .map((w) => w[0] ?? '')
      .join(''),
  );
</script>

{#if src && src !== failed}
  <img
    class="icon {kind} {tier ?? ''}"
    class:dim
    style:--frame={frame}
    {src}
    alt={name}
    title={name}
    width={size}
    height={size}
    decoding="async"
    onerror={() => (failed = src)}
  />
{:else}
  <span
    class="icon tile {kind} {tier ?? ''}"
    class:dim
    style:--frame={frame}
    style:width="{size}px"
    style:height="{size}px"
    style:font-size="{Math.round(size * 0.38)}px"
    title={name}>{initials}</span
  >
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
  .tile {
    display: inline-grid;
    place-items: center;
    font-family: var(--cond);
    font-weight: 600;
    color: var(--muted);
    overflow: hidden;
  }
  .unit {
    border: 2px solid var(--frame, var(--line));
  }
  .item {
    box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.5);
  }
  .trait {
    background: none;
    border-radius: 0;
  }
  .augment.silver {
    box-shadow: 0 0 0 1px #aab4be;
  }
  .augment.gold {
    box-shadow: 0 0 0 1px var(--gold);
  }
  .augment.prismatic {
    box-shadow: 0 0 0 1px #c9a6ff;
  }
  .dim {
    filter: grayscale(1) brightness(0.6);
    opacity: 0.75;
  }
</style>
