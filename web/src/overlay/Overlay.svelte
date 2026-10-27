<script lang="ts">
  // Drawn over the game window, click-through. Sizes follow the window height (vh) so it reads
  // the same at 1080p and 1440p. LoL strip above the minimap, TFT strip top left.
  import { clockText } from '../lib/ui';
  import { overlay } from './store.svelte';

  const ORD = ['1st', '2nd', '3rd', '4th'];
  const v = $derived(overlay.view);
</script>

{#if v}
  {#each v.badges as b, i (i)}
    <div class="badge" class:top={b.place === 0} style:left="{b.x * 100}%" style:top="{v.badge_y * 100}%">
      <strong>{b.place == null ? '–' : ORD[b.place]}</strong>
      {#if b.reason}<span>{b.reason}</span>{/if}
    </div>
  {/each}

  {#if v.head || v.call || v.timers.length}
    <section class="strip" class:tft={v.tft}>
      {#if v.head}
        <p class="head">{v.head}{#if v.detail}<span class="detail">{v.detail}</span>{/if}</p>
      {/if}
      {#if v.call}<p class="call">{v.call}</p>{/if}
      {#if v.timers.length}
        <p class="timers">
          {#each v.timers as t (t.name)}
            {@const left = t.next_spawn == null ? 0 : t.next_spawn - overlay.time}
            <span>{t.name} <b class:soon={left > 0 && left <= 60} class:up={left <= 0}>{left > 0 ? clockText(left) : 'up'}</b></span>
          {/each}
        </p>
      {/if}
    </section>
  {/if}
{/if}

<style>
  :global(html),
  :global(body) {
    margin: 0;
    height: 100%;
    overflow: hidden;
    background: transparent;
    color: #ebe6d9;
    font: 600 max(12px, 1.35vh) / 1.25 'Barlow Condensed', 'Arial Narrow', sans-serif;
    -webkit-font-smoothing: antialiased;
    user-select: none;
    pointer-events: none;
  }
  .badge,
  .strip {
    position: fixed;
    background: rgba(12, 16, 22, 0.78);
    border: 1px solid rgba(220, 183, 92, 0.35);
    border-radius: 0.4vh;
    text-shadow: 0 1px 2px #000;
  }
  .badge {
    transform: translateX(-50%);
    display: flex;
    align-items: baseline;
    gap: 0.6vh;
    max-width: 17%;
    padding: 0.3vh 0.9vh;
    white-space: nowrap;
  }
  .badge strong {
    font-size: 1.5em;
    font-weight: 700;
    color: #8f9bab;
  }
  .badge.top {
    border-color: #dcb75c;
  }
  .badge.top strong {
    color: #dcb75c;
  }
  .badge span {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: #c9c3b4;
  }
  .strip {
    right: 0.6%;
    bottom: 27%;
    max-width: 22%;
    padding: 0.5vh 1vh;
    display: grid;
    gap: 0.3vh;
  }
  .strip.tft {
    right: auto;
    bottom: auto;
    left: 1%;
    top: 9%;
  }
  p {
    margin: 0;
  }
  .head {
    color: #dcb75c;
  }
  .detail {
    margin-left: 0.8vh;
    color: #c9c3b4;
  }
  .call {
    border-left: 0.3vh solid #dcb75c;
    padding-left: 0.6vh;
  }
  .timers {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4vh 1.2vh;
    color: #8f9bab;
  }
  .timers b {
    color: #ebe6d9;
    font-variant-numeric: tabular-nums;
  }
  .timers .soon {
    color: #dcb75c;
  }
  .timers .up {
    color: #5fc4a9;
  }
</style>
