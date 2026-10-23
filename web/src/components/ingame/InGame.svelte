<script lang="ts">
  import type { GameAdvice } from '../../lib/bindings/GameAdvice';
  import AugmentOffer from './AugmentOffer.svelte';
  import BuyNext from './BuyNext.svelte';
  import BuildPath from './BuildPath.svelte';
  import Threats from './Threats.svelte';
  import SelfCoach from './SelfCoach.svelte';
  import Matchups from './Matchups.svelte';
  import Timers from './Timers.svelte';
  import GoldDiff from './GoldDiff.svelte';
  import Shopping from './Shopping.svelte';
  import DeathWindows from './DeathWindows.svelte';
  import Defense from './Defense.svelte';
  import MyAugments from './MyAugments.svelte';
  import GoldSinks from './GoldSinks.svelte';
  import TabHint from './TabHint.svelte';
  import Calls from './Calls.svelte';
  import Readiness from './Readiness.svelte';
  import Bounties from './Bounties.svelte';

  let { game }: { game: GameAdvice } = $props();
  const rift = $derived(game.mode === 'rift');
  const laneOpp = $derived(game.threats.find((t) => t.lane_opp)?.champ ?? null);
</script>

<div class="ingame" class:rift>
  {#if game.calls.length}<div class="calls"><Calls calls={game.calls} /></div>{/if}
  {#if game.augments || game.my_augments.length}
    <div class="aug">
      {#if game.augments}<AugmentOffer advice={game.augments} champ={game.my_champ} />{/if}
      {#if game.my_augments.length}<MyAugments picks={game.my_augments} />{/if}
    </div>
  {/if}
  <div class="build">
    <Shopping list={game.shopping} at={game.time} />
    <BuyNext pick={game.buy_next} alternatives={game.alternatives} eta={game.gold_eta} upgrade={game.support_upgrade} />
    {#if game.gold_sinks.length}<GoldSinks sinks={game.gold_sinks} />{/if}
    <BuildPath owned={game.owned} path={game.path} />
  </div>
  <div class="threats">
    {#if game.tab_hint}<TabHint hint={game.tab_hint} />{/if}
    {#if game.readiness}<Readiness r={game.readiness} />{/if}
    <Threats threats={game.threats} />
    {#if game.bounties.length}<Bounties bounties={game.bounties} />{/if}
    <DeathWindows deaths={game.deaths} at={game.time} />
  </div>
  <div class="me"><SelfCoach me={game.me} {laneOpp} /></div>
  {#if game.defense}<div class="def"><Defense d={game.defense} /></div>{/if}
  <div class="gold"><GoldDiff g={game.gold} /></div>
  {#if rift}
    <div class="side">
      <Matchups matchups={game.matchups} />
      <Timers timers={game.timers} />
    </div>
  {/if}
</div>

<style>
  .ingame {
    display: grid;
    gap: var(--gap);
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    grid-template-areas:
      'calls calls'
      'aug aug'
      'build threats'
      'me me'
      'def def'
      'gold gold'
      'side side';
    align-content: start;
  }
  .calls {
    grid-area: calls;
  }
  .aug {
    grid-area: aug;
    display: flex;
    flex-direction: column;
    gap: var(--gap);
  }
  .def {
    grid-area: def;
    min-width: 0;
  }
  .build {
    grid-area: build;
    display: flex;
    flex-direction: column;
    gap: var(--gap);
    min-width: 0;
  }
  .threats {
    grid-area: threats;
    display: flex;
    flex-direction: column;
    gap: var(--gap);
    min-width: 0;
  }
  .me {
    grid-area: me;
  }
  .gold {
    grid-area: gold;
    min-width: 0;
  }
  .side {
    grid-area: side;
    display: grid;
    grid-template-columns: minmax(0, 2fr) minmax(0, 1fr);
    gap: var(--gap);
  }
  /* Wide screens (Rift): matchups and timers get their own column on the right. */
  @media (min-width: 1400px) {
    .ingame.rift {
      grid-template-columns: minmax(0, 6fr) minmax(0, 5fr) minmax(0, 3fr);
      grid-template-areas:
        'calls calls calls'
        'aug aug aug'
        'build threats side'
        'me me side'
        'def def side'
        'gold gold side';
    }
    .side {
      grid-template-columns: 1fr;
      align-content: start;
    }
  }
  @media (max-width: 760px) {
    .ingame {
      grid-template-columns: 1fr;
      grid-template-areas: 'calls' 'aug' 'build' 'threats' 'me' 'def' 'gold' 'side';
    }
  }
</style>
