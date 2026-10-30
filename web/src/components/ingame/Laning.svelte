<script lang="ts">
  // Laning panel: tips against my lane opponent(s). After laning it shrinks to one line
  // per opponent (spells to play around) until opened again.
  import type { MatchupCard as Card } from '../../lib/bindings/MatchupCard';
  import { store } from '../../lib/store.svelte';
  import { champName, cooldownText } from '../../lib/ui';
  import Icon from '../Icon.svelte';
  import MatchupCard from '../MatchupCard.svelte';

  let { cards, me, at }: { cards: Card[]; me: number; at: number } = $props();

  /** Laning ends around 14:00 (first tower plates fall). */
  const LANING_END_S = 14 * 60;
  let opened = $state(false);
  const open = $derived(at < LANING_END_S || opened);
  const name = (c: number) => (store.ui ? champName(store.ui, c) : '');
</script>

<section class="panel">
  <h2>
    Laning vs {cards.map((c) => name(c.champ)).join(' + ')}
    {#if at >= LANING_END_S}<button class="toggle" onclick={() => (opened = !opened)}>{open ? 'Hide tips' : 'Show tips'}</button>{/if}
  </h2>
  {#if open}
    <div class="cards">
      {#each cards as c (c.champ)}<MatchupCard card={c} {me} />{/each}
    </div>
  {:else}
    <ul class="short">
      {#each cards as c (c.champ)}
        <li>
          <Icon kind="champ" id={c.champ} size={24} />
          <span class="who">{name(c.champ)}</span>
          {#each c.spells.filter((s) => s.watch) as s (s.key)}
            <span class="num watch">{s.key} {s.now != null ? `${Math.round(s.now)}s` : cooldownText(s.cooldown)}</span>
          {/each}
          {#if c.note}<span class="muted trade">{c.note.trade}</span>{/if}
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  h2 {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .toggle {
    margin-left: auto;
    padding: 1px 8px;
    border: 1px solid var(--line);
    border-radius: var(--r);
    background: none;
    color: var(--muted);
    font-size: 12px;
    cursor: pointer;
  }
  .cards {
    display: grid;
    gap: 14px;
  }
  .short {
    display: grid;
    gap: 4px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .short li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .who {
    font: 600 15px var(--cond);
  }
  .watch {
    color: var(--warn);
    font-weight: 600;
  }
  .trade {
    font-size: 13px;
  }
</style>
