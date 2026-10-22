<script lang="ts">
  // Offered augments as a ranked list (u.gg order for this champion). No win rates and no
  // single "pick this" directive (Riot third-party policy); the top-ranked one is only tinted.
  import type { AugmentAdvice } from '../../lib/bindings/AugmentAdvice';
  import type { AugmentRank } from '../../lib/bindings/AugmentRank';
  import { store } from '../../lib/store.svelte';
  import { champName } from '../../lib/ui';
  import Icon from '../Icon.svelte';

  let { advice, champ }: { advice: AugmentAdvice; champ: number } = $props();

  const name = $derived(store.ui ? champName(store.ui, champ) : '');
  const ranked = $derived(
    [...advice.offered].sort((a, b) => (a.rank ?? Infinity) - (b.rank ?? Infinity) || (a.id === advice.best ? -1 : 0)),
  );
  const rankText = (a: AugmentRank) =>
    a.rank == null
      ? `Not ranked for ${name}`
      : `#${a.rank + 1} of ${a.of}${a.rarity ? ` ${a.rarity[0].toUpperCase()}${a.rarity.slice(1)}` : ''} for ${name}`;
</script>

<section class="panel" aria-label="Augment offer">
  <h2>Augment offer, ranked for {name}</h2>
  <ol>
    {#each ranked as a, i (a.id)}
      <li class:top={i === 0 && a.rank != null}>
        <span class="pos num">#{i + 1}</span>
        <Icon kind="augment" id={a.id} size={44} title={a.name} />
        <div>
          <div class="name">{a.name}</div>
          <div class="rank">{rankText(a)}</div>
          {#if a.synergy}<div class="syn">{a.synergy}</div>{/if}
        </div>
      </li>
    {/each}
  </ol>
</section>

<style>
  ol {
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 10px;
  }
  li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    border-radius: var(--r);
    background: var(--raised);
  }
  li.top {
    box-shadow: inset 0 0 0 1px var(--gold-dim);
  }
  .pos {
    font-size: 22px;
    font-weight: 700;
    color: var(--muted);
  }
  .top .pos {
    color: var(--gold);
  }
  .name {
    font: 600 20px/1.1 var(--cond);
  }
  .rank {
    font-size: 14px;
    color: var(--muted);
  }
  .syn {
    font-size: 14px;
    color: var(--good);
  }
</style>
