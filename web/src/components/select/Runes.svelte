<script lang="ts">
  import type { RuneAdvice } from '../../lib/bindings/RuneAdvice';
  import type { RunePick } from '../../lib/bindings/RunePick';
  import { gold, pct, runeIcon } from '../../lib/ui';

  let { runes }: { runes: RuneAdvice } = $props();
  const swapped = $derived([...runes.primary, ...runes.secondary, ...runes.shards].filter((r) => r.reason));
</script>

{#snippet rune(r: RunePick, size: number)}
  <li class:swapped={r.reason}>
    <img src={runeIcon(r.icon)} alt="" width={size} height={size} />
    <span class="name">{r.name}</span>
  </li>
{/snippet}

<section class="panel runes">
  <h2>Runes <span class="num">· {pct(runes.win_rate)} over {gold(runes.games)} games</span> <span class="src">{runes.source}</span></h2>
  <div class="page">
    <ol class="primary">
      {#each runes.primary as r, i (`${i}-${r.id}`)}{@render rune(r, i === 0 ? 48 : 30)}{/each}
    </ol>
    <ol>
      {#each runes.secondary as r, i (`${i}-${r.id}`)}{@render rune(r, 30)}{/each}
    </ol>
    <ol class="shards">
      {#each runes.shards as r, i (`${i}-${r.id}`)}{@render rune(r, 22)}{/each}
    </ol>
  </div>
  {#if runes.summoners.length}
    <p class="spells muted">Summoners: <span class="name">{runes.summoners.join(' + ')}</span></p>
  {/if}
  {#each swapped as r, i (`${i}-${r.id}`)}
    <p class="why"><span class="name">{r.name}</span> <span class="muted">{r.reason}</span></p>
  {/each}
  {#if runes.alt}
    <p class="alt muted">
      Highest win rate:
      <img src={runeIcon(runes.alt.keystone.icon)} alt="" width="22" height="22" />
      <span class="name">{runes.alt.keystone.name}</span>
      <span class="num">{pct(runes.alt.win_rate)} over {gold(runes.alt.games)} games</span>
    </p>
  {/if}
</section>

<style>
  h2 .num {
    font-weight: 500;
  }
  .src {
    float: right;
    font: 500 12px/1.6 var(--sans);
    color: var(--faint);
  }
  .spells {
    margin: 10px 0 0;
  }
  .page {
    display: grid;
    grid-template-columns: minmax(0, 1.2fr) minmax(0, 1fr) minmax(0, 1fr);
    gap: 20px;
  }
  ol {
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    gap: 6px;
    align-content: start;
  }
  li {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  img {
    flex: none;
    border-radius: 50%;
    background: var(--raised);
    vertical-align: middle;
  }
  .name {
    font: 600 17px/1.1 var(--cond);
  }
  .primary li:first-child .name {
    font-size: 21px;
  }
  .shards .name {
    font: 500 14px/1.1 var(--sans);
    color: var(--muted);
  }
  .swapped img {
    box-shadow: 0 0 0 2px var(--gold);
  }
  .swapped .name {
    color: var(--gold);
  }
  .why,
  .alt {
    margin: 10px 0 0;
    font-size: 14px;
  }
  .why .name {
    color: var(--gold);
  }
  .alt {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }
  .alt .name {
    color: var(--text);
  }
  @media (max-width: 760px) {
    .page {
      grid-template-columns: 1fr;
      gap: 10px;
    }
  }
</style>
