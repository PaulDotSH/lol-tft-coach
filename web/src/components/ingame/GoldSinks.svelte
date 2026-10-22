<script lang="ts">
  // Full build: options for spare gold, each with its reason. Options, not orders.
  import type { GoldSink } from '../../lib/bindings/GoldSink';
  import Icon from '../Icon.svelte';

  let { sinks }: { sinks: GoldSink[] } = $props();
  const LABEL: Record<string, string> = { anvil: 'Stat Anvil', elixir: 'Elixir', sell_boots: 'Sell boots', swap: 'Swap' };
</script>

<section class="panel" aria-label="Spare gold">
  <h2>Spare gold</h2>
  <ul>
    {#each sinks as s (s.kind)}
      <li>
        <span class="icons">
          {#if s.sell != null}
            <span class="sold"><Icon kind="item" id={s.sell} size={30} /></span>
            <span class="arrow" aria-hidden="true">→</span>
          {/if}
          {#if s.item != null}<Icon kind="item" id={s.item} size={36} />{/if}
        </span>
        <div class="text">
          <div class="head">
            <span class="kind">{LABEL[s.kind] ?? s.kind}</span>
            <span class="num gold">{s.cost.toLocaleString('en-US')} g</span>
          </div>
          <div class="reason muted">{s.reason}</div>
        </div>
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
    gap: 10px;
  }
  li {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .icons {
    display: flex;
    align-items: center;
    gap: 4px;
    flex: none;
    min-width: 36px;
  }
  .sold {
    opacity: 0.5;
  }
  .arrow {
    color: var(--faint);
  }
  .text {
    min-width: 0;
  }
  .head {
    display: flex;
    gap: 10px;
    align-items: baseline;
  }
  .kind {
    font: 600 19px var(--cond);
  }
  .gold {
    color: var(--gold);
    font-size: 17px;
  }
  .reason {
    font-size: 15px;
    line-height: 1.3;
  }
</style>
