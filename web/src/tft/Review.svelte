<script lang="ts">
  // Post-game review of my last TFT game: placement, the comp it matched (and that comp's
  // average), my final board with stars and items, active traits, and takeaways.
  import type { TftReview } from '../lib/tft-bindings/TftReview';
  import { tft } from '../lib/tft-store.svelte';
  import { avgText, placeClass, placeText, tftName, tftQueueName } from '../lib/tft';
  import { clockText } from '../lib/ui';
  import TftIcon from './TftIcon.svelte';

  let { review }: { review: TftReview } = $props();

  const g = $derived(review.game);
  const units = $derived([...g.units].sort((a, b) => b.items.length - a.items.length || b.star - a.star));
  const traits = $derived([...g.traits].filter(([, , tier]) => tier > 0).sort((a, b) => b[2] - a[2] || b[1] - a[1]));
</script>

<section class="panel review" aria-label="Last game">
  <h2>Last game · {tftQueueName(g.queue, g.mode)}</h2>
  <div class="top">
    <span class="place num {placeClass(g.placement)}">{placeText(g.placement)}</span>
    <div class="facts">
      {#if review.comp}
        <span class="cname">{review.comp.name}</span>
        <span class="muted">
          comp average <b class="num">{avgText(review.comp.avg_place)}</b>
          {#if review.comp.levelling}· plays {review.comp.levelling}{/if}
        </span>
      {:else}
        <span class="cname muted">No meta comp matched</span>
      {/if}
      <span class="muted small">level {g.level} · round {g.last_round} · {clockText(g.duration_s)}</span>
    </div>
  </div>

  {#if review.takeaways.length}
    <ul class="takeaways">
      {#each review.takeaways as t, i (i)}<li>{t}</li>{/each}
    </ul>
  {/if}

  <h3>Final board</h3>
  <ul class="board">
    {#each units as u, i (`${u.api}-${i}`)}
      <li>
        <TftIcon kind="unit" api={u.api} size={44} />
        <span class="stars s{u.star}">{'★'.repeat(u.star)}</span>
        <span class="uname">{tftName(tft.set, 'unit', u.api)}</span>
        {#if u.items.length}
          <span class="items">
            {#each u.items as it, j (`${it}-${j}`)}<TftIcon kind="item" api={it} size={18} />{/each}
          </span>
        {/if}
      </li>
    {/each}
  </ul>

  {#if traits.length}
    <h3>Traits</h3>
    <ul class="traits">
      {#each traits as [t, n, tier] (t)}
        <li class="tier{Math.min(tier, 4)}">
          <TftIcon kind="trait" api={t} size={18} />
          <span class="num">{n}</span>
          {tftName(tft.set, 'trait', t)}
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .top {
    display: flex;
    align-items: center;
    gap: 18px;
  }
  .place {
    font-size: 56px;
    font-weight: 700;
    line-height: 1;
  }
  .first {
    color: var(--gold);
  }
  .top4 {
    color: var(--good);
  }
  .bot4 {
    color: var(--threat);
  }
  .facts {
    display: grid;
    gap: 2px;
    min-width: 0;
  }
  .cname {
    font: 600 24px/1.1 var(--cond);
  }
  .small {
    font-size: 13px;
  }
  .takeaways {
    margin: 12px 0 0;
    padding-left: 18px;
    display: grid;
    gap: 4px;
    font-size: 16px;
  }
  h3 {
    margin: 14px 0 6px;
    font: 600 14px var(--cond);
    color: var(--muted);
  }
  .board {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .board li {
    width: 66px;
    display: grid;
    justify-items: center;
    gap: 1px;
  }
  .stars {
    font-size: 11px;
    line-height: 1;
    color: var(--muted);
  }
  .stars.s2 {
    color: #c9d1db;
  }
  .stars.s3 {
    color: var(--gold);
  }
  .uname {
    max-width: 100%;
    font: 500 12px/1.1 var(--cond);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .items {
    display: flex;
    gap: 1px;
  }
  .traits {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .traits li {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 2px 8px 2px 4px;
    border-radius: 4px;
    background: var(--raised);
    font: 500 14px var(--cond);
  }
  .tier1 {
    box-shadow: inset 3px 0 0 #a0715e;
  }
  .tier2 {
    box-shadow: inset 3px 0 0 #aab4be;
  }
  .tier3 {
    box-shadow: inset 3px 0 0 var(--gold);
  }
  .tier4 {
    box-shadow: inset 3px 0 0 #c9a6ff;
  }
</style>
