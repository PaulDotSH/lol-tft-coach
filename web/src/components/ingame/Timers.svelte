<script lang="ts">
  import type { ObjectiveTimer } from '../../lib/bindings/ObjectiveTimer';
  import { clock } from '../../lib/store.svelte';
  import { clockText } from '../../lib/ui';

  let { timers }: { timers: ObjectiveTimer[] } = $props();
</script>

<section class="panel">
  <h2>Objectives</h2>
  {#if timers.length === 0}<p class="empty">No timers yet.</p>{/if}
  <ul>
    {#each timers as t (t.name)}
      {@const left = t.next_spawn == null ? 0 : t.next_spawn - clock.time}
      <li>
        <span>{t.name}</span>
        {#if left > 0}
          <span class="num t" class:soon={left <= 60}>{clockText(left)}</span>
        {:else}
          <span class="num t up">up</span>
        {/if}
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
    gap: 4px;
  }
  li {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }
  .t {
    font-size: 22px;
    font-weight: 600;
  }
  .soon {
    color: var(--gold);
  }
  .up {
    color: var(--good);
  }
</style>
