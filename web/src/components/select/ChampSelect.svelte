<script lang="ts">
  import type { ChampSelectAdvice } from '../../lib/bindings/ChampSelectAdvice';
  import { store } from '../../lib/store.svelte';
  import { champName, LANE_NAMES } from '../../lib/ui';
  import Icon from '../Icon.svelte';
  import Suggestions from './Suggestions.svelte';
  import BuildPlan from './BuildPlan.svelte';
  import Bench from './Bench.svelte';
  import DraftPanel from './DraftPanel.svelte';
  import TeamSide from './TeamSide.svelte';

  let { cs }: { cs: ChampSelectAdvice } = $props();
  const rift = $derived(cs.mode === 'rift');
</script>

<div class="cs">
  <section class="you panel">
    {#if cs.my_champ != null}
      <Icon kind="champ" id={cs.my_champ} size={56} />
      <div>
        <div class="name">{store.ui ? champName(store.ui, cs.my_champ) : ''}</div>
        <div class="muted">{cs.my_lane ? LANE_NAMES[cs.my_lane] : rift ? 'Role not assigned' : 'Your champion'}</div>
      </div>
    {:else}
      <div class="name muted">No champion hovered yet</div>
    {/if}
    {#if cs.enemies.length}
      <div class="enemies">
        <h2>Enemies revealed</h2>
        <div class="icons">
          {#each cs.enemies as e (e)}<Icon kind="champ" id={e} size={36} />{/each}
        </div>
      </div>
    {/if}
  </section>

  {#if cs.bench.length || cs.team}
    <div class="pair">
      {#if cs.bench.length}<Bench bench={cs.bench} />{/if}
      {#if cs.team}<section class="panel"><TeamSide title="Your team" side={cs.team} icons /></section>{/if}
    </div>
  {/if}

  {#if cs.picks.length || cs.bans.length}
    <div class="pair">
      <Suggestions title="Suggested picks" items={cs.picks} />
      <Suggestions title="Suggested bans" items={cs.bans} ban />
    </div>
  {/if}

  {#if cs.draft}
    <DraftPanel draft={cs.draft} />
  {/if}

  {#if cs.build}
    <BuildPlan plan={cs.build} />
  {:else if !cs.picks.length}
    <p class="empty">Build plan appears once you lock in.</p>
  {/if}
</div>

<style>
  .cs {
    display: grid;
    gap: var(--gap);
    align-content: start;
  }
  .you {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .name {
    font: 700 28px/1.05 var(--cond);
  }
  .enemies {
    margin-left: auto;
  }
  .enemies h2 {
    margin-bottom: 4px;
  }
  .icons {
    display: flex;
    gap: 6px;
  }
  .pair {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: var(--gap);
  }
  @media (max-width: 760px) {
    .pair {
      grid-template-columns: 1fr;
    }
  }
</style>
