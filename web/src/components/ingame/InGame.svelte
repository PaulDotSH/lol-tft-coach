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

  let { game }: { game: GameAdvice } = $props();
  const rift = $derived(game.mode === 'rift');
  const laneOpp = $derived(game.threats.find((t) => t.lane_opp)?.champ ?? null);
</script>

<div class="ingame" class:rift>
  {#if game.augments}
    <div class="aug"><AugmentOffer advice={game.augments} champ={game.my_champ} /></div>
  {/if}
  <div class="build">
    <Shopping list={game.shopping} at={game.time} />
    <BuyNext pick={game.buy_next} alternatives={game.alternatives} eta={game.gold_eta} />
    <BuildPath owned={game.owned} path={game.path} />
  </div>
  <div class="threats">
    <Threats threats={game.threats} />
    <DeathWindows deaths={game.deaths} at={game.time} />
  </div>
  <div class="me"><SelfCoach me={game.me} {laneOpp} /></div>
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
      'aug aug'
      'build threats'
      'me me'
      'gold gold'
      'side side';
    align-content: start;
  }
  .aug {
    grid-area: aug;
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
        'aug aug aug'
        'build threats side'
        'me me side'
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
      grid-template-areas: 'aug' 'build' 'threats' 'me' 'gold' 'side';
    }
  }
</style>
