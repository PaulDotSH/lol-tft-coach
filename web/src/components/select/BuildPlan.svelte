<script lang="ts">
  import type { BuildPlan } from '../../lib/bindings/BuildPlan';
  import type { ItemPick } from '../../lib/bindings/ItemPick';
  import Icon from '../Icon.svelte';

  let { plan }: { plan: BuildPlan } = $props();
</script>

{#snippet pick(p: ItemPick)}
  <li>
    <Icon kind="item" id={p.item} size={48} title={p.name} />
    <div>
      <div class="name">{p.name}</div>
      {#if p.reason}<div class="reason muted">{p.reason}</div>{/if}
    </div>
  </li>
{/snippet}

<section class="panel plan">
  <div>
    <h2>Start</h2>
    <div class="start">
      {#each plan.start as id, i (`${i}-${id}`)}<Icon kind="item" {id} size={40} />{/each}
    </div>
    {#if plan.upgrade}
      <div class="upgrade" title={plan.upgrade.reason}>
        <span class="muted">Upgrade to</span>
        <Icon kind="item" id={plan.upgrade.item} size={28} />
        <span class="uname">{plan.upgrade.name}</span>
      </div>
    {/if}
  </div>
  <div>
    <h2>Core <span class="src">{plan.source}</span></h2>
    <ol>
      {#each plan.core as p (p.item)}{@render pick(p)}{/each}
    </ol>
  </div>
  {#if plan.boots}
    <div>
      <h2>Boots</h2>
      <ol>{@render pick(plan.boots)}</ol>
    </div>
  {/if}
</section>

<style>
  .src {
    margin-left: 6px;
    font: 500 12px/1 var(--sans);
    color: var(--faint);
  }
  .plan {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) minmax(0, 0.6fr);
    gap: 24px;
  }
  .start {
    display: flex;
    gap: 6px;
  }
  .upgrade {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 8px;
    font-size: 13px;
  }
  .uname {
    font: 600 16px/1.1 var(--cond);
  }
  ol {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-wrap: wrap;
    gap: 10px 20px;
  }
  li {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .name {
    font: 600 19px/1.1 var(--cond);
  }
  .reason {
    font-size: 13px;
  }
  @media (max-width: 900px) {
    .plan {
      grid-template-columns: 1fr;
      gap: 12px;
    }
  }
</style>
