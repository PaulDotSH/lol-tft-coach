<script lang="ts">
  // My latest run of games: record, the mistakes that repeat across them, goals met.
  import type { PastGame } from '../../lib/bindings/PastGame';
  import type { Session } from '../../lib/bindings/Session';
  import { GOALS, targetText } from '../../lib/goals';
  import Icon from '../Icon.svelte';

  let { session, games }: { session: Session; games: PastGame[] } = $props();

  /** Ended this recently: still "tonight". */
  const TONIGHT_MS = 6 * 3600 * 1000;
  const title = $derived(
    Date.now() - session.end < TONIGHT_MS
      ? 'Tonight'
      : `Last session · ${new Date(session.start).toLocaleDateString(undefined, { month: 'short', day: 'numeric' })}`,
  );
  const played = $derived(session.game_ids.flatMap((id) => games.filter((g) => g.game_id === id)));
</script>

<section class="panel">
  <div class="top">
    <h2>{title}</h2>
    <span class="num rec"><span class="good">{session.wins}W</span> <span class="bad">{session.losses}L</span></span>
  </div>
  <div class="champs">
    {#each played as g (g.game_id)}
      <span class="champ" class:win={g.win} title="{g.kills}/{g.deaths}/{g.assists}"><Icon kind="champ" id={g.champ} size={30} /></span>
    {/each}
  </div>
  {#if session.repeats.length}
    <h3>Again and again</h3>
    <ul>
      {#each session.repeats as r (r.text)}
        <li><span>{r.text}</span><span class="num muted">{r.games} of {r.of} games</span></li>
      {/each}
    </ul>
  {/if}
  {#if session.goals.length}
    <h3>Goals</h3>
    <ul>
      {#each session.goals as s (s.goal.kind)}
        <li class:met={s.met === s.of}>
          <span>{GOALS[s.goal.kind].label} {targetText(s.goal.kind, s.goal.target)}</span>
          <span class="num muted">{s.met} of {s.of} games</span>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .top {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
  }
  .rec {
    font-size: 15px;
    font-weight: 600;
  }
  .champs {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .champ {
    display: inline-flex;
    border-radius: 4px;
    border-bottom: 3px solid var(--threat);
  }
  .champ.win {
    border-bottom-color: var(--good);
  }
  h3 {
    margin: 12px 0 4px;
    font-size: 13px;
    font-weight: 500;
    color: var(--muted);
  }
  ul {
    display: grid;
    gap: 3px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    padding: 5px 10px;
    border-left: 3px solid var(--threat);
    border-radius: 4px;
    background: var(--raised);
  }
  li.met {
    border-left-color: var(--good);
  }
</style>
