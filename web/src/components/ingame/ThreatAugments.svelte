<script lang="ts">
  // Mayhem: enemy augments read off the Tab scoreboard, with their rank in that champion's
  // u.gg list (ranks only, never win rates).
  import type { Threat } from '../../lib/bindings/Threat';
  import type { AugmentRank } from '../../lib/bindings/AugmentRank';
  import Icon from '../Icon.svelte';

  let { t }: { t: Threat } = $props();
  const title = (a: AugmentRank) =>
    a.rank == null
      ? `${a.name} (not ranked for ${t.name})`
      : `${a.name} · #${a.rank + 1} of ${a.of} ${a.rarity ?? ''} for ${t.name}`;
</script>

{#if t.augments.length}
  <span class="augs" aria-label="Augments">
    {#each t.augments as a, i (i)}
      <span class="aug {a.rarity ?? ''}" title={title(a)}>
        <Icon kind="augment" id={a.id} size={20} title={title(a)} />
        {#if a.rank != null}<span class="num">#{a.rank + 1}</span>{/if}
      </span>
    {/each}
  </span>
{/if}

<style>
  .augs {
    display: flex;
    margin-top: 3px;
    flex-wrap: wrap;
    gap: 4px;
  }
  .aug {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 0 5px 0 1px;
    border-radius: 3px;
    background: var(--raised);
    font-size: 12px;
    line-height: 20px;
    color: var(--muted);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .prismatic {
    box-shadow: inset 0 0 0 1px rgb(178 120 200 / 0.7);
  }
  .gold {
    box-shadow: inset 0 0 0 1px var(--gold-dim);
  }
</style>
