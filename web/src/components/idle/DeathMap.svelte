<script lang="ts" module>
  export type Spot = { x: number; y: number; tone: 'win' | 'loss' | 'plain'; title: string };
  export type Seen = { x: number; y: number; label: string; title: string };
</script>

<script lang="ts">
  // Summoner's Rift minimap with death markers; positions are seen from blue side.
  import { store } from '../../lib/store.svelte';

  let {
    spots,
    size = 260,
    seen = [],
    link = null,
  }: { spots: Spot[]; size?: number; seen?: Seen[]; link?: [{ x: number; y: number }, { x: number; y: number }] | null } = $props();

  // Data Dragon map bounds for map11.png.
  const MIN = { x: -120, y: -120 };
  const MAX = { x: 14870, y: 14980 };
  const px = (x: number) => ((x - MIN.x) / (MAX.x - MIN.x)) * 100;
  const py = (y: number) => (1 - (y - MIN.y) / (MAX.y - MIN.y)) * 100;
  const src = $derived(store.ui ? `https://ddragon.leagueoflegends.com/cdn/${store.ui.version}/img/map/map11.png` : '');
</script>

<div class="map" style:width="{size}px" style:height="{size}px">
  {#if src}<img {src} alt="" width={size} height={size} />{/if}
  {#if link}
    <svg viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
      <line x1={px(link[0].x)} y1={py(link[0].y)} x2={px(link[1].x)} y2={py(link[1].y)} vector-effect="non-scaling-stroke" />
    </svg>
  {/if}
  {#each spots as s, i (i)}
    <span class="spot {s.tone}" style:left="{px(s.x)}%" style:top="{py(s.y)}%" title={s.title}></span>
  {/each}
  {#each seen as s, i (i)}
    <span class="seen" style:left="{px(s.x)}%" style:top="{py(s.y)}%" title={s.title}><i>{s.label}</i></span>
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
  svg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }
  line {
    stroke: var(--warn);
    stroke-width: 1.5;
    stroke-dasharray: 3 3;
  }
  .seen {
    position: absolute;
    width: 7px;
    height: 7px;
    margin: -3.5px 0 0 -3.5px;
    border-radius: 50%;
    background: var(--warn);
    box-shadow: 0 0 0 1.5px rgb(0 0 0 / 0.6);
  }
  .seen i {
    position: absolute;
    left: 8px;
    top: -5px;
    font: 600 10px var(--cond);
    font-style: normal;
    white-space: nowrap;
    color: var(--warn);
    text-shadow: 0 0 2px #000;
  }
</style>
