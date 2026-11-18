<script lang="ts">
  import type { BuildPace } from '../../lib/bindings/BuildPace';
  import type { GoldEta as Eta } from '../../lib/bindings/GoldEta';
  import type { ItemPick } from '../../lib/bindings/ItemPick';
  import type { WhatIf } from '../../lib/bindings/WhatIf';
  import { store } from '../../lib/store.svelte';
  import { clockText, itemName } from '../../lib/ui';
  import Icon from '../Icon.svelte';
  import GoldEta from './GoldEta.svelte';
  import WhatIfFigure from './WhatIfFigure.svelte';

  let {
    pick,
    alternatives,
    eta = null,
    upgrade = null,
    whatIf = [],
    pace = null,
  }: { pick: ItemPick | null; alternatives: ItemPick[]; eta?: Eta | null; upgrade?: ItemPick | null; whatIf?: WhatIf[]; pace?: BuildPace | null } = $props();
  const wi = (item: number) => whatIf.find((w) => w.item === item);
  const component = $derived(pick?.component_now ?? null);
  const componentCost = $derived(component != null ? (store.ui?.items[component]?.cost ?? null) : null);
</script>

<section class="buy" aria-label="Buy next">
  {#if pick}
    <div class="main">
      <span class="frame"><Icon kind="item" id={pick.item} size={88} title={pick.name} /></span>
      <div class="text">
        <div class="name">{pick.name}</div>
        <div class="cost num">{pick.cost.toLocaleString()} g</div>
        {#if pick.reason}<p class="reason">{pick.reason}</p>{/if}
        <WhatIfFigure w={wi(pick.item)} />
        {#if eta && eta.item === pick.item}<GoldEta {eta} />{/if}
        {#if pace}
          <p class="pace muted" title="Ranked Emerald+ games on this champion and role: half finish it between {clockText(pace.early)} and {clockText(pace.late)}">
            {pace.slot}: Emerald+ by {clockText(pace.median)}
          </p>
        {/if}
      </div>
    </div>
    {#if component != null}
      <div class="now">
        <span class="muted" title="Not enough gold for the full item yet">Buy now</span>
        <Icon kind="item" id={component} size={28} />
        <strong class="cname">{store.ui ? itemName(store.ui, component) : ''}</strong>
        {#if componentCost != null}<span class="num gold">{componentCost} g</span>{/if}
      </div>
    {/if}
  {:else}
    <p class="empty">Full build. Nothing left to buy.</p>
  {/if}
  {#if upgrade}
    <div class="now upgrade" title={upgrade.reason}>
      <span class="muted">Quest upgrade</span>
      <Icon kind="item" id={upgrade.item} size={28} />
      <strong class="cname">{upgrade.name}</strong>
      <span class="alt-reason muted">{upgrade.reason}</span>
    </div>
  {/if}

  {#if alternatives.length}
    <ul class="alts" aria-label="Alternatives">
      {#each alternatives as alt (alt.item)}
        <li>
          <Icon kind="item" id={alt.item} size={40} title={alt.name} />
          <span class="alt-name">{alt.name}</span>
          <span class="num gold">{alt.cost.toLocaleString()}</span>
          <span class="alt-reason muted">{alt.reason}</span>
          <span class="alt-fig"><WhatIfFigure w={wi(alt.item)} /></span>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .buy {
    background: linear-gradient(180deg, #2a2a26 0%, var(--panel) 70%);
    border: 1px solid var(--gold-dim);
    border-radius: 10px;
    padding: 16px 18px 12px;
  }
  .main {
    display: flex;
    gap: 18px;
    align-items: flex-start;
  }
  .frame {
    display: inline-flex;
    padding: 3px;
    border-radius: 7px;
    background: linear-gradient(135deg, var(--gold), var(--gold-dim));
  }
  .frame :global(.icon) {
    border-radius: 5px;
  }
  .text {
    min-width: 0;
  }
  .name {
    font: 700 40px / 1 var(--cond);
    color: var(--text);
  }
  .cost {
    margin-top: 4px;
    font-size: 24px;
    font-weight: 600;
    color: var(--gold);
  }
  .reason {
    margin: 6px 0 0;
    font-size: 19px;
    line-height: 1.3;
  }
  .now {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 12px;
    padding: 6px 10px;
    border-radius: var(--r);
    background: rgb(0 0 0 / 0.2);
    font-size: 16px;
  }
  .now > * {
    white-space: nowrap;
  }
  .cname {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .gold {
    color: var(--gold);
    font-size: 17px;
  }
  .alts {
    margin: 14px 0 0;
    padding: 10px 0 0;
    border-top: 1px solid var(--line);
    list-style: none;
    display: grid;
    gap: 6px;
  }
  .alts li {
    display: grid;
    grid-template-columns: 40px auto auto 1fr auto;
    align-items: center;
    column-gap: 10px;
  }
  .pace {
    margin: 6px 0 0;
    font-size: 15px;
  }
  .text :global(.whatif) {
    margin-top: 6px;
  }
  .alt-name {
    font: 600 19px var(--cond);
  }
  .alt-reason {
    font-size: 14px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
