<script lang="ts" module>
  export type Spot = { x: number; y: number; tone: 'win' | 'loss' | 'plain'; title: string };
</script>

<script lang="ts">
  // Summoner's Rift minimap with death markers; positions are seen from blue side.
  import { store } from '../../lib/store.svelte';

  let { spots, size = 260 }: { spots: Spot[]; size?: number } = $props();

  // Data Dragon map bounds for map11.png.
  const MIN = { x: -120, y: -120 };
  const MAX = { x: 14870, y: 14980 };
  const px = (x: number) => ((x - MIN.x) / (MAX.x - MIN.x)) * 100;
  const py = (y: number) => (1 - (y - MIN.y) / (MAX.y - MIN.y)) * 100;
  const src = $derived(store.ui ? `https://ddragon.leagueoflegends.com/cdn/${store.ui.version}/img/map/map11.png` : '');
</script>

<div class="map" style:width="{size}px" style:height="{size}px">
  {#if src}<img {src} alt="" width={size} height={size} />{/if}
  {#each spots as s, i (i)}
    <span class="spot {s.tone}" style:left="{px(s.x)}%" style:top="{py(s.y)}%" title={s.title}></span>
  {/each}
</div>

<style>
  .map {
    position: relative;
    flex: none;
    border-radius: var(--r);
    overflow: hidden;
    background: var(--raised);
  }
  img {
    display: block;
    width: 100%;
    height: 100%;
    filter: saturate(0.5) brightness(0.7);
  }
  .spot {
    position: absolute;
    width: 9px;
    height: 9px;
    margin: -4.5px 0 0 -4.5px;
    border-radius: 50%;
    border: 1.5px solid rgb(0 0 0 / 0.6);
    background: var(--gold);
    opacity: 0.85;
  }
  .spot.loss {
    background: var(--threat);
  }
  .spot.win {
    background: var(--good);
  }
</style>
