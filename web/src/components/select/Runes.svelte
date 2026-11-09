<script lang="ts">
  import type { RuneAdvice } from '../../lib/bindings/RuneAdvice';
  import type { RuneOption } from '../../lib/bindings/RuneOption';
  import type { RunePick } from '../../lib/bindings/RunePick';
  import { gold, pct, runeIcon } from '../../lib/ui';

  let { runes }: { runes: RuneAdvice } = $props();
  const key = (o: RuneOption) => `${o.label}|${o.source}`;
  // Kept by name: a matchup page arriving later goes in front without moving the choice.
  let chosen = $state<string | null>(null);
  const page = $derived(runes.options.find((o) => key(o) === chosen) ?? runes.options[0]);
  const swapped = $derived([...page.primary, ...page.secondary, ...page.shards].filter((r) => r.reason));
</script>

{#snippet rune(r: RunePick, size: number)}
  <li class:swapped={r.reason}>
    <img src={runeIcon(r.icon)} alt="" width={size} height={size} />
    <span class="name">{r.name}</span>
  </li>
{/snippet}

<section class="panel runes">
  <h2>Runes</h2>
  {#if runes.options.length > 1}
    <div class="options" role="tablist" aria-label="Rune pages">
      {#each runes.options as o (key(o))}
        <button type="button" role="tab" aria-selected={o === page} class:on={o === page} onclick={() => (chosen = key(o))}>
          {#if o.primary[0]}<img src={runeIcon(o.primary[0].icon)} alt="" width="26" height="26" />{/if}
          <span class="what">
            <span class="label" class:vs={o.vs !== null}>{o.label}</span>
            <span class="src">{o.source}</span>
          </span>
          <span class="num wr">{pct(o.win_rate)}</span>
        </button>
      {/each}
    </div>
  {/if}
  <p class="meta muted">
    {#if runes.options.length === 1}<span class="label">{page.label}</span> ·{/if}
    <span class="num">{pct(page.win_rate)} over {gold(page.games)} games</span>
    {#if runes.options.length === 1}<span class="src">{page.source}</span>{/if}
  </p>
  <div class="page">
    <ol class="primary">
      {#each page.primary as r, i (`${i}-${r.id}`)}{@render rune(r, i === 0 ? 48 : 30)}{/each}
    </ol>
    <ol>
      {#each page.secondary as r, i (`${i}-${r.id}`)}{@render rune(r, 30)}{/each}
    </ol>
    <ol class="shards">
      {#each page.shards as r, i (`${i}-${r.id}`)}{@render rune(r, 22)}{/each}
    </ol>
  </div>
  {#if page.also.length}
    <p class="also muted">Same page as {page.also.join(', ')}</p>
  {/if}
  {#if runes.summoners.length}
    <p class="spells muted">Summoners: <span class="name">{runes.summoners.join(' + ')}</span></p>
  {/if}
  {#each swapped as r, i (`${i}-${r.id}`)}
    <p class="why"><span class="name">{r.name}</span> <span class="muted">{r.reason}</span></p>
  {/each}
</section>

<style>
  .options {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 10px;
  }
  .options button {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    padding: 4px 10px 4px 4px;
    border: 1px solid var(--line);
    border-radius: var(--r);
    background: none;
    color: var(--muted);
    cursor: pointer;
    text-align: left;
  }
  .options button:hover {
    background: var(--raised);
  }
  .options .on {
    background: var(--raised);
    border-color: var(--gold-dim);
    color: var(--text);
  }
  .what {
    display: grid;
    min-width: 0;
  }
  .label {
    font: 600 15px/1.1 var(--cond);
  }
  .label.vs {
    color: var(--gold);
  }
  .src {
    font: 500 12px/1.3 var(--sans);
    color: var(--faint);
  }
  .wr {
    font-size: 15px;
  }
  .meta {
    margin: 0 0 10px;
    font-size: 14px;
  }
  .meta .src {
    float: right;
  }
  .spells,
  .also {
    margin: 10px 0 0;
  }
  .also {
    font-size: 14px;
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
  .why {
    margin: 10px 0 0;
    font-size: 14px;
  }
  .why .name {
    color: var(--gold);
  }
  @media (max-width: 760px) {
    .page {
      grid-template-columns: 1fr;
      gap: 10px;
    }
  }
</style>
