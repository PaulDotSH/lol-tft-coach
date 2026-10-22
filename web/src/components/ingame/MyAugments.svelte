<script lang="ts">
  // Augments taken this game. Detected from stat changes when an offer closes; when the
  // pick can't be told apart the remaining candidates are shown instead of a guess.
  import type { MyAugment } from '../../lib/bindings/MyAugment';
  import { store } from '../../lib/store.svelte';
  import Icon from '../Icon.svelte';

  let { picks }: { picks: MyAugment[] } = $props();
  const name = (id: number) => store.ui?.augments[id] ?? `#${id}`;
</script>

<section class="panel" aria-label="Your augments">
  <h2>Your augments</h2>
  <ol>
    {#each picks as p, i (i)}
      <li>
        {#if p.id != null}
          <Icon kind="augment" id={p.id} size={34} />
          <span class="name">{name(p.id)}</span>
        {:else}
          <span class="unsure" title="Could not tell which card was taken">
            {#each p.candidates as c (c)}<Icon kind="augment" id={c} size={26} />{/each}
          </span>
          <span class="name muted">{p.candidates.map(name).join(' / ')}?</span>
        {/if}
      </li>
    {/each}
  </ol>
</section>

<style>
  ol {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-wrap: wrap;
    gap: 8px 22px;
  }
  li {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .name {
    font: 600 18px var(--cond);
  }
  .unsure {
    display: flex;
    gap: 2px;
    opacity: 0.6;
  }
</style>
