<script lang="ts">
  // Recent games from the League client's match history; click one to review it.
  import type { PastGame } from '../../lib/bindings/PastGame';
  import { store } from '../../lib/store.svelte';
  import { ago, champName, clockText, LANE_NAMES, queueName } from '../../lib/ui';
  import Icon from '../Icon.svelte';

  let { games, connected, onopen }: { games: PastGame[]; connected: boolean; onopen: (id: number) => void } = $props();
</script>

<section class="panel">
  <h2>Match history</h2>
  {#if !games.length}
    <p class="empty">{connected ? 'Reading your match history…' : 'Connect the League client to see your recent games.'}</p>
  {:else}
    <ul>
      {#each games as g (g.game_id)}
        <li>
          <button type="button" class="row" class:win={g.win && !g.remake} class:loss={!g.win && !g.remake} onclick={() => onopen(g.game_id)}>
            <Icon kind="champ" id={g.champ} size={34} />
            <span class="who">
              <span class="name">{store.ui ? champName(store.ui, g.champ) : ''}</span>
              <span class="muted small">
                {queueName(g.queue_id, g.mode)}{g.lane ? ` · ${LANE_NAMES[g.lane]}` : ''} · {ago(g.created)}
              </span>
            </span>
            <span class="res small {g.remake ? 'muted' : g.win ? 'good' : 'bad'}">{g.remake ? 'Remake' : g.win ? 'Win' : 'Loss'}</span>
            <span class="kda num">{g.kills}/{g.deaths}/{g.assists}</span>
            <span class="len num muted">{clockText(g.duration_s)}</span>
            <span class="go" aria-hidden="true">›</span>
          </button>
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
  .row {
    width: 100%;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) 46px 64px 40px 10px;
    align-items: center;
    gap: 10px;
    padding: 5px 8px 5px 6px;
    border: 0;
    border-left: 3px solid var(--line);
    border-radius: 4px;
    background: var(--raised);
    text-align: left;
    cursor: pointer;
  }
  .row:hover {
    background: color-mix(in srgb, var(--raised) 80%, var(--text));
  }
  .win {
    border-left-color: var(--good);
  }
  .loss {
    border-left-color: var(--threat);
  }
  .who {
    display: grid;
    min-width: 0;
  }
  .name {
    font: 600 16px/1.15 var(--cond);
  }
  .who .small {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .small {
    font-size: 13px;
  }
  .res {
    font-weight: 600;
  }
  .kda {
    font-size: 17px;
    text-align: right;
  }
  .len {
    text-align: right;
  }
  .go {
    color: var(--faint);
    font-size: 18px;
  }
  .row:hover .go {
    color: var(--gold);
  }
</style>
