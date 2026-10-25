<script lang="ts">
  // My recent TFT games from the League client: placement, the comp it matched,
  // the board (itemised units first), level and when.
  import type { TftPastGame } from '../lib/tft-bindings/TftPastGame';
  import { tft } from '../lib/tft-store.svelte';
  import { placeClass, placeText, tftName, tftQueueName } from '../lib/tft';
  import { ago } from '../lib/ui';
  import TftIcon from './TftIcon.svelte';

  let { games, names }: { games: TftPastGame[]; names: Map<string, string> } = $props();

  /** The comp's name, else the two biggest traits of the board. */
  const compName = (g: TftPastGame) =>
    (g.comp && names.get(g.comp)) ||
    [...g.traits]
      .sort((a, b) => b[2] - a[2] || b[1] - a[1])
      .slice(0, 2)
      .map(([t, n]) => `${n} ${tftName(tft.set, 'trait', t)}`)
      .join(' · ') ||
    'Unknown comp';
  const board = (g: TftPastGame) => [...g.units].sort((a, b) => b.items.length - a.items.length || b.star - a.star).slice(0, 8);
</script>

<section class="panel">
  <h2>Recent games</h2>
  {#if !games.length}
    <p class="empty">Your TFT games appear here once the League client is connected.</p>
  {:else}
    <ul>
      {#each games as g (g.game_id)}
        <li class={placeClass(g.placement)}>
          <span class="place num">{placeText(g.placement)}</span>
          <span class="who">
            <span class="name">{compName(g)}</span>
            <span class="muted small">{tftQueueName(g.queue, g.mode)} · level {g.level} · {ago(g.created)}</span>
          </span>
          <span class="board">
            {#each board(g) as u, i (`${u.api}-${i}`)}
              <span class="u" title="{tftName(tft.set, 'unit', u.api)} {'★'.repeat(u.star)}">
                <TftIcon kind="unit" api={u.api} size={28} />
                {#if u.star > 1}<span class="st s{u.star}">{u.star}</span>{/if}
              </span>
            {/each}
          </span>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    gap: 3px;
  }
  li {
    display: grid;
    grid-template-columns: 44px minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    padding: 5px 8px;
    border-left: 3px solid var(--line);
    border-radius: 4px;
    background: var(--raised);
  }
  .first {
    border-left-color: var(--gold);
  }
  .top4 {
    border-left-color: var(--good);
  }
  .bot4 {
    border-left-color: var(--threat);
  }
  .place {
    font-size: 20px;
    font-weight: 700;
  }
  .first .place {
    color: var(--gold);
  }
  .top4 .place {
    color: var(--good);
  }
  .bot4 .place {
    color: var(--threat);
  }
  .who {
    display: grid;
    min-width: 0;
  }
  .name {
    font: 600 16px/1.15 var(--cond);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .small {
    font-size: 13px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .board {
    display: flex;
    gap: 3px;
  }
  .u {
    position: relative;
  }
  .st {
    position: absolute;
    right: -2px;
    bottom: -3px;
    padding: 0 3px;
    border-radius: 3px;
    background: var(--bg);
    font: 700 11px/1.2 var(--cond);
    color: #c9d1db;
  }
  .st.s3 {
    color: var(--gold);
  }
  @media (max-width: 760px) {
    .board {
      display: none;
    }
  }
</style>
