<script lang="ts">
  // Build rows from the same item engine: for the game (my champion's stats and my matchup pages
  // against the revealed enemies) and for the lane (my page against my lane opponent alone).
  import type { BuildPlan } from '../../lib/bindings/BuildPlan';
  import type { ItemPick } from '../../lib/bindings/ItemPick';
  import Icon from '../Icon.svelte';

  /** `title` is empty outside the Rift; `plan` is null while a row has nothing yet (`note` says why). */
  let { rows }: { rows: { title: string; plan: BuildPlan | null; note?: string }[] } = $props();
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

<section class="panel">
  {#each rows as r (r.title)}
    {@const plan = r.plan}
    {#if r.title}
      <h2 class="row">
        {r.title}
        {#if plan?.vs.length}
          <span class="vs">vs {#each plan.vs as c (c)}<Icon kind="champ" id={c} size={22} />{/each}</span>
        {/if}
        {#if plan}<span class="src">{plan.source}</span>{/if}
      </h2>
    {/if}
    {#if plan}
      <div class="plan">
        <div>
          <h3>Start</h3>
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
          <h3>Core{#if !r.title}<span class="src">{plan.source}</span>{/if}</h3>
          <ol>
            {#each plan.core as p (p.item)}{@render pick(p)}{/each}
          </ol>
        </div>
        {#if plan.boots}
          <div>
            <h3>Boots</h3>
            <ol>{@render pick(plan.boots)}</ol>
          </div>
        {/if}
      </div>
    {:else if r.note}
      <p class="empty">{r.note}</p>
    {/if}
  {/each}
</section>

<style>
  .src {
    margin-left: 6px;
    font: 500 12px/1 var(--sans);
    letter-spacing: 0;
    text-transform: none;
    color: var(--faint);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .plan + .row,
  .empty + .row {
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid var(--line);
  }
  .vs {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font: 500 12px/1 var(--sans);
    letter-spacing: 0;
    text-transform: none;
    color: var(--muted);
  }
  h3 {
    margin: 0 0 6px;
    font: 600 12px/1 var(--sans);
    letter-spacing: 0.06em;
    text-transform: uppercase;
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
