<script lang="ts">
  // Live TFT advice: calls, augment offer, the gold line, the shop, target comp, items,
  // my units with contested ones, and the Choncc panel in that mode.
  import type { TftAdvice } from '../../lib/tft-bindings/TftAdvice';
  import Augments from './Augments.svelte';
  import Board from './Board.svelte';
  import Calls from './Calls.svelte';
  import Choncc from './Choncc.svelte';
  import Econ from './Econ.svelte';
  import Items from './Items.svelte';
  import Shop from './Shop.svelte';
  import Target from './Target.svelte';

  let { advice }: { advice: TftAdvice } = $props();
</script>

<div class="ingame">
  {#if advice.calls.length}<div class="calls"><Calls calls={advice.calls} stage={advice.stage} /></div>{/if}
  {#if advice.augments.length}<div class="aug"><Augments augments={advice.augments} /></div>{/if}
  <div class="econ"><Econ econ={advice.econ} stage={advice.stage} team={advice.team_size} /></div>
  <div class="shop"><Shop shop={advice.shop} /></div>
  <div class="comp"><Target comps={advice.comps} /></div>
  <div class="side">
    <Items items={advice.items} />
    <Board owned={advice.owned} contested={advice.contested} />
  </div>
  {#if advice.choncc}<div class="choncc"><Choncc choncc={advice.choncc} /></div>{/if}
</div>

<style>
  .ingame {
    display: grid;
    gap: var(--gap);
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    grid-template-areas:
      'calls calls'
      'aug aug'
      'econ econ'
      'shop shop'
      'comp side'
      'choncc side';
    align-content: start;
  }
  .calls {
    grid-area: calls;
  }
  .aug {
    grid-area: aug;
  }
  .econ {
    grid-area: econ;
  }
  .shop {
    grid-area: shop;
  }
  .comp {
    grid-area: comp;
    min-width: 0;
  }
  .side {
    grid-area: side;
    display: flex;
    flex-direction: column;
    gap: var(--gap);
    min-width: 0;
  }
  .choncc {
    grid-area: choncc;
    min-width: 0;
  }
  @media (max-width: 760px) {
    .ingame {
      grid-template-columns: 1fr;
      grid-template-areas: 'calls' 'aug' 'econ' 'shop' 'comp' 'choncc' 'side';
    }
  }
</style>
