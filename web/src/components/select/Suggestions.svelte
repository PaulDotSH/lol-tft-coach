<script lang="ts">
  import type { PickSuggestion } from '../../lib/bindings/PickSuggestion';
  import { store } from '../../lib/store.svelte';
  import { champName } from '../../lib/ui';
  import Icon from '../Icon.svelte';

  // `note`: what the list is ranked against; `empty`: what to say while it has no rows.
  let { title, items, ban = false, note = '', empty = 'Nothing to suggest yet.' }: { title: string; items: PickSuggestion[]; ban?: boolean; note?: string; empty?: string } = $props();
</script>

<section class="panel" class:ban>
  <h2>{title}{#if note}<span class="note">{note}</span>{/if}</h2>
  {#if items.length === 0}<p class="empty">{empty}</p>{/if}
  <ul>
    {#each items as s (s.champ)}
      <li>
        <Icon kind="champ" id={s.champ} size={48} />
        <div>
          <div class="name">{store.ui ? champName(store.ui, s.champ) : ''}</div>
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
  .note {
    margin-left: 6px;
    font: 500 12px/1 var(--sans);
    letter-spacing: 0;
    text-transform: none;
    color: var(--faint);
  }
  .name {
    font: 600 22px/1.1 var(--cond);
  }
  .reason {
    font-size: 14px;
  }
  .ban :global(.icon) {
    filter: grayscale(0.6);
    box-shadow: 0 0 0 2px var(--threat);
  }
</style>
