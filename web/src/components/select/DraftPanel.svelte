<script lang="ts">
  // Both teams' draft as a breakdown: lane matchups, damage mix, frontline/engage/CC,
  // gaps and the factors behind each side's estimate. Shows options, never a verdict.
  import type { DraftReport } from '../../lib/bindings/DraftReport';
  import { store } from '../../lib/store.svelte';
  import { champName, LANE_NAMES, pct } from '../../lib/ui';
  import Icon from '../Icon.svelte';
  import TeamSide from './TeamSide.svelte';
  import { tone } from './draft';

  let { draft }: { draft: DraftReport } = $props();

  const name = (k: number) => (store.ui ? champName(store.ui, k) : '');
</script>

<section class="panel draft">
  {#if draft.lanes.length}
    <ol class="lanes">
      {#each draft.lanes as l (l.lane)}
        <li title={l.ally_wr != null ? `${l.games.toLocaleString()} games` : ''}>
          <span class="lane muted">{LANE_NAMES[l.lane]}</span>
          <span class="pair">
            {#if l.ally != null}<Icon kind="champ" id={l.ally} size={30} />{:else}<span class="slot"></span>{/if}
            {#if l.ally_wr != null}
              <span class="num wr {tone(l.ally_wr - 50, 2.5)}">{pct(l.ally_wr)}</span>
            {:else}
              <span class="vs muted">vs</span>
            {/if}
            {#if l.enemy != null}
              <span class="enemy" title="Likely {LANE_NAMES[l.lane]}: {name(l.enemy)}"><Icon kind="champ" id={l.enemy} size={30} /></span>
            {:else}<span class="slot"></span>{/if}
          </span>
        </li>
      {/each}
    </ol>
  {/if}
  <div class="sides">
    <TeamSide title="Your team" side={draft.allies} />
    <TeamSide title="Enemy team" side={draft.enemies} />
  </div>
  <p class="note">Win-rate points vs an even draft, from lolalytics. Enemy lanes are a guess from play rates.</p>
</section>

<style>
  .draft {
    display: grid;
    gap: 12px;
  }
  .lanes {
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(118px, 1fr));
    gap: 8px;
  }
  .lanes li {
    display: grid;
    justify-items: center;
    gap: 2px;
    padding: 4px 0 6px;
    border-radius: var(--r);
    background: var(--raised);
  }
  .lane {
    font: 600 12px/1.2 var(--cond);
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
  .pair {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .wr {
    min-width: 46px;
    text-align: center;
    font-size: 17px;
    font-weight: 600;
  }
  .vs {
    min-width: 46px;
    text-align: center;
    font-size: 12px;
  }
  .enemy :global(.icon) {
    box-shadow: 0 0 0 2px var(--threat);
  }
  .slot {
    width: 30px;
    height: 30px;
    border-radius: 50%;
    border: 1px dashed var(--line);
  }
  .sides {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 20px;
  }
  .note {
    margin: 0;
    font-size: 12px;
    color: var(--faint);
  }
  @media (max-width: 760px) {
    .sides {
      grid-template-columns: 1fr;
    }
  }
</style>
