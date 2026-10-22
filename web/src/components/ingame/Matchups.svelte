<script lang="ts">
  import type { MatchupTip } from '../../lib/bindings/MatchupTip';
  import { store } from '../../lib/store.svelte';
  import { champName, pct } from '../../lib/ui';
  import Icon from '../Icon.svelte';

  let { matchups }: { matchups: MatchupTip[] } = $props();
  const TONE: Record<string, string> = { favoured: 'good', unfavoured: 'bad' };
</script>

<section class="panel">
  <h2>Your win rate vs</h2>
  {#if matchups.length === 0}<p class="empty">No matchup data.</p>{/if}
  <ul>
    {#each matchups as m (m.champ)}
      <li class:opp={m.lane_opp} title="{m.games.toLocaleString()} games">
        <Icon kind="champ" id={m.champ} size={26} />
        <span class="name">{store.ui ? champName(store.ui, m.champ) : ''}</span>
        <span class="num wr {TONE[m.label] ?? ''}">{pct(m.vs_wr)}</span>
        <span class="label {TONE[m.label] ?? 'muted'}">{m.label}</span>
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
    grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
    gap: 6px 16px;
  }
  li {
    display: grid;
    grid-template-columns: 26px 1fr auto auto;
    align-items: center;
    gap: 8px;
    padding: 2px 6px 2px 2px;
    border-radius: var(--r);
  }
  .opp {
    background: var(--raised);
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .wr {
    font-size: 18px;
    font-weight: 600;
  }
  .label {
    font-size: 12px;
    width: 64px;
  }
</style>
