<script lang="ts">
  // Champ select: my past games against each enemy revealed so far.
  import type { MatchupMemory } from '../../lib/bindings/MatchupMemory';
  import { store } from '../../lib/store.svelte';
  import { champName, gold, signed } from '../../lib/ui';
  import Icon from '../Icon.svelte';

  let { memory }: { memory: MatchupMemory[] } = $props();
  const name = (c: number) => (store.ui ? champName(store.ui, c) : '');
</script>

<section class="panel">
  <h2>Your history vs them</h2>
  <ul>
    {#each memory as m (m.champ)}
      <li>
        <Icon kind="champ" id={m.champ} size={30} />
        <span class="who">{name(m.champ)}</span>
        <span class="num rec">
          <span class="good">{m.wins}W</span>
          <span class="bad">{m.games - m.wins}L</span>
        </span>
        <span class="muted small">
          {#if m.lane_games}
            lane {m.lane_games}×{#if m.gold_15 != null}, <span class:good={m.gold_15 > 0} class:bad={m.gold_15 < 0}>{signed(m.gold_15, gold)}</span> at 15{/if}
            ·
          {/if}
          {#if m.killed_me}<span class:warn={m.killed_me >= 2 * m.games}>killed you {m.killed_me}×</span>{:else}never killed you{/if}
        </span>
      </li>
    {/each}
  </ul>
</section>

<style>
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    gap: 6px;
  }
  li {
    display: grid;
    grid-template-columns: auto 110px 70px 1fr;
    align-items: center;
    gap: 10px;
  }
  .who {
    font: 600 16px var(--cond);
  }
  .rec {
    font-size: 16px;
  }
  .small {
    font-size: 13px;
  }
  .warn {
    color: var(--warn);
  }
</style>
