<script lang="ts">
  // One of the two overlay windows, click-through: `?part=strip` (Buy next, call, timers) or
  // `?part=cards` (rank badges across the offer cards). Each window is only as big as its part,
  // so sizes use --u, 1 % of the game window's height (`?h=` is this window's share of it).
  import { clockText } from '../lib/ui';
  import { overlay } from './store.svelte';

  const ORD = ['1st', '2nd', '3rd', '4th'];
  const q = new URLSearchParams(location.search);
  const part = q.get('part') ?? 'strip';
  const share = Number(q.get('h')) || 1;
  const v = $derived(overlay.view);
</script>

<main style:--u="calc(1vh / {share})">
  {#if v && part === 'cards'}
    {#each v.badges as b, i (i)}
      <div class="badge" class:top={b.place === 0} style:left="{b.x * 100}%">
        <strong>{b.place == null ? '–' : ORD[b.place]}</strong>
        {#if b.reason}<span>{b.reason}</span>{/if}
      </div>
    {/each}
  {:else if v && part === 'strip' && (v.head || v.call || v.timers.length)}
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
</main>

<style>
  :global(html),
  :global(body) {
    margin: 0;
    height: 100%;
    overflow: hidden;
    background: transparent;
    user-select: none;
    pointer-events: none;
  }
  main {
    height: 100%;
    color: #ebe6d9;
    font: 600 max(12px, calc(var(--u) * 1.35)) / 1.25 'Barlow Condensed', 'Arial Narrow', sans-serif;
    -webkit-font-smoothing: antialiased;
  }
  .badge,
  .strip {
    position: fixed;
    background: rgba(12, 16, 22, 0.78);
    border: 1px solid rgba(220, 183, 92, 0.35);
    border-radius: calc(var(--u) * 0.4);
    text-shadow: 0 1px 2px #000;
  }
  .badge {
    top: 0;
    transform: translateX(-50%);
    display: flex;
    align-items: baseline;
    gap: calc(var(--u) * 0.6);
    max-width: 24%;
    padding: calc(var(--u) * 0.3) calc(var(--u) * 0.9);
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
    right: 0;
    bottom: 0;
    max-width: calc(100% - 2px);
    max-height: calc(100% - 2px);
    box-sizing: border-box;
    overflow: hidden;
    padding: calc(var(--u) * 0.5) calc(var(--u) * 1);
    display: grid;
    gap: calc(var(--u) * 0.3);
  }
  .strip.tft {
    right: auto;
    bottom: auto;
    left: 0;
    top: 0;
  }
  p {
    margin: 0;
  }
  .head {
    color: #dcb75c;
  }
  .detail {
    margin-left: calc(var(--u) * 0.8);
    color: #c9c3b4;
  }
  .call {
    border-left: calc(var(--u) * 0.3) solid #dcb75c;
    padding-left: calc(var(--u) * 0.6);
  }
  .timers {
    display: flex;
    flex-wrap: wrap;
    gap: calc(var(--u) * 0.4) calc(var(--u) * 1.2);
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
