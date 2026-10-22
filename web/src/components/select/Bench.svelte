<script lang="ts">
  import type { BenchRank } from '../../lib/bindings/BenchRank';
  import { store } from '../../lib/store.svelte';
  import { champName } from '../../lib/ui';
  import Icon from '../Icon.svelte';

  let { bench }: { bench: BenchRank[] } = $props();
  const mineAt = $derived(bench.findIndex((b) => b.mine));
  const tierClass = (t: string | null) => (t ? (t.startsWith('S') ? 's' : t === 'A' ? 'a' : t === 'B' ? 'b' : 'c') : 'c');
</script>

<section class="panel">
  <h2>
    {mineAt === 0 ? 'Your champion is the best available' : mineAt > 0 ? `${mineAt} better option${mineAt > 1 ? 's' : ''} on the bench` : 'Bench'}
  </h2>
  <ol>
    {#each bench as b (b.champ)}
      <li class:mine={b.mine}>
        <Icon kind="champ" id={b.champ} size={52} />
        <span class="tier num {tierClass(b.tier)}">{b.tier ?? '?'}</span>
        <span class="name">{store.ui ? champName(store.ui, b.champ) : ''}</span>
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
    gap: 14px;
  }
  li {
    position: relative;
    display: grid;
    justify-items: center;
    gap: 4px;
    width: 76px;
    padding: 6px 0;
    border-radius: var(--r);
  }
  .mine {
    background: var(--raised);
  }
  .mine :global(.icon) {
    box-shadow: 0 0 0 2px var(--text);
  }
  .tier {
    position: absolute;
    top: 0;
    right: 6px;
    min-width: 26px;
    padding: 0 4px;
    border-radius: 4px;
    font-size: 15px;
    font-weight: 700;
    text-align: center;
    background: var(--line);
  }
  .s {
    background: var(--gold);
    color: #2a2210;
  }
  .a {
    background: var(--good);
    color: #10261f;
  }
  .b {
    background: #6b7f96;
  }
  .name {
    font-size: 13px;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
