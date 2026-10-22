<script lang="ts">
  // Chips on a threat row: R rank by visible level with its static base cooldown, finished legendaries.
  import type { Threat } from '../../lib/bindings/Threat';
  import { store } from '../../lib/store.svelte';
  import { itemName } from '../../lib/ui';
  import Icon from '../Icon.svelte';

  let { t }: { t: Threat } = $props();
  const u = $derived(t.ult);
  const cd = $derived(u && u.cooldowns.length >= u.rank ? u.cooldowns[u.rank - 1] : null);
  const ultTitle = $derived(
    u
      ? `R rank ${u.rank} possible at their level` +
          (u.cooldowns.length ? ` · base cooldown ${u.cooldowns.join('/')} s (static data, before haste)` : '')
      : '',
  );
</script>

{#if u}
  <span class="chip ult" class:hot={u.spike} title={ultTitle}>
    {u.spike ? (u.rank === 1 ? 'R online' : `R${u.rank} now`) : `R${u.rank}`}{#if cd != null}<span class="num"> · {cd} s</span>{/if}
  </span>
{/if}
{#if t.legendaries > 0}
  <span class="chip" class:up={t.legendary_diff > 0} title="Finished legendaries ({t.legendary_diff >= 0 ? '+' : '−'}{Math.abs(t.legendary_diff)} vs lobby average)">
    {t.legendaries} item{t.legendaries === 1 ? '' : 's'}
  </span>
{/if}
{#each t.new_items as id (id)}
  <span class="chip new" title="Just finished {store.ui ? itemName(store.ui, id) : ''}">
    <Icon kind="item" {id} size={16} />new
  </span>
{/each}

<style>
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 0 6px;
    border-radius: 3px;
    background: var(--raised);
    font-size: 12px;
    line-height: 18px;
    color: var(--muted);
  }
  .ult {
    color: var(--text);
  }
  .hot {
    background: rgb(228 104 93 / 0.22);
    color: var(--threat);
    font-weight: 600;
  }
  .up {
    color: var(--warn);
  }
  .new {
    padding-left: 1px;
    background: rgb(224 162 74 / 0.18);
    color: var(--warn);
    font-weight: 600;
  }
</style>
