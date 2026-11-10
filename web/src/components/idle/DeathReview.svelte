<script lang="ts">
  // Death review (recap): each death with killer, gold, what Buy next said, and enemy level spikes.
  import type { DeathReview } from '../../lib/bindings/DeathReview';
  import { store } from '../../lib/store.svelte';
  import { champName, clockText, gold, signed } from '../../lib/ui';
  import Icon from '../Icon.svelte';

  let { review, picked = null, onpick }: { review: DeathReview; picked?: number | null; onpick?: (i: number) => void } = $props();
  const HOLDING = 1000;
  const name = (c: number) => (store.ui ? champName(store.ui, c) : '');
  // Match-history reviews of games the app did not watch have no Buy next.
  const buyCol = $derived(review.deaths.some((d) => d.buy_next != null));
  const cols = $derived(buyCol ? 5 : 4);
</script>

<div class="review">
  <h2>Deaths</h2>
  {#if review.notes.length}
    <ul class="notes">
      {#each review.notes as n, i (i)}<li>{n}</li>{/each}
    </ul>
  {/if}
  {#if !review.deaths.length}
    <p class="empty">No deaths recorded this game.</p>
  {:else}
    <table>
      <thead>
        <tr><th>Time</th><th>By</th><th>Gold vs killer</th><th>Held</th>{#if buyCol}<th>Buy next</th>{/if}</tr>
      </thead>
      <tbody>
        {#each review.deaths as d, i (i)}
          <tr class:picked={picked === i}>
            <td class="num">
              {#if onpick && d.pos}
                <button type="button" class="pick" title="Show on the map" onclick={() => onpick(i)}>{clockText(d.time)}</button>
              {:else}
                {clockText(d.time)}
              {/if}
            </td>
            <td class="by">
              {#if d.killer != null}
                <Icon kind="champ" id={d.killer} size={26} />
              {:else}
                <span class="muted">Executed</span>
              {/if}
              {#each d.assisters as a, j (`${j}-${a}`)}<span class="assist"><Icon kind="champ" id={a} size={18} /></span>{/each}
              {#if d.place}<span class="tag">{d.place}</span>{/if}
              {#if d.traded}<span class="tag trade" title="Your team won the fight around this death">trade</span>{/if}
              {#if d.killer_top_threat}<span class="tag">top threat</span>{/if}
              {#each d.spikes as [c, lvl], j (`${j}-${c}-${lvl}`)}
                <span class="tag spike" title="{name(c)} reached level {lvl} within 60 s before">{name(c)} {lvl}</span>
              {/each}
            </td>
            <td class="num" class:bad={!d.traded && d.gold_gap != null && d.gold_gap < 0}>{d.gold_gap != null ? signed(d.gold_gap, gold) : ''}</td>
            <td class="num" class:warn={d.unspent >= HOLDING}>{gold(d.unspent)}</td>
            {#if buyCol}<td>
              {#if d.buy_next != null}
                <span class="buy" class:afford={d.could_buy} title={d.could_buy ? 'You could afford it (or its component)' : ''}>
                  <Icon kind="item" id={d.buy_next} size={24} />
                </span>
              {/if}
            </td>{/if}
          </tr>
          {#if d.sight?.line}
            <tr class="sight {d.sight.verdict}" class:picked={picked === i}>
              <td></td>
              <td colspan={cols - 1}>
                <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
                  {#if d.sight.verdict === 'warned'}
                    <path d="M8 1.5 15 14H1z" /><path class="cut" d="M8 6v4M8 11.5v1" />
                  {:else if d.sight.verdict === 'fog'}
                    <path d="M4.5 12a3 3 0 0 1 .4-6 4 4 0 0 1 7.6 1.3A2.4 2.4 0 0 1 12 12z" />
                  {:else}
                    <path d="M1 8s2.6-5 7-5 7 5 7 5-2.6 5-7 5-7-5-7-5z" /><circle class="cut" cx="8" cy="8" r="2" />
                  {/if}
                </svg>
                {d.sight.line}
              </td>
            </tr>
          {/if}
        {/each}
      </tbody>
    </table>
  {/if}
</div>

<style>
  .review {
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid var(--line);
  }
  .notes {
    margin: 0 0 10px;
    padding-left: 18px;
    color: var(--warn);
  }
  .notes li + li {
    margin-top: 3px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 14px;
  }
  th {
    text-align: left;
    font: 600 12px var(--cond);
    color: var(--faint);
    padding-bottom: 4px;
  }
  td {
    padding: 3px 8px 3px 0;
    vertical-align: middle;
  }
  .by {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
  }
  .assist {
    display: inline-flex;
    opacity: 0.75;
  }
  .tag {
    padding: 0 6px;
    border-radius: 999px;
    background: var(--raised);
    font-size: 12px;
    color: var(--muted);
  }
  .spike {
    color: var(--warn);
  }
  .trade {
    color: var(--good);
  }
  .warn {
    color: var(--warn);
  }
  .buy {
    display: inline-flex;
    border-radius: 4px;
  }
  .afford {
    box-shadow: 0 0 0 2px var(--warn);
  }
  .pick {
    padding: 0;
    border: 0;
    background: none;
    color: var(--gold);
    font: inherit;
    text-decoration: underline dotted;
    cursor: pointer;
  }
  tr.picked {
    background: var(--raised);
  }
  .sight td {
    padding-top: 0;
    font-size: 13px;
    color: var(--muted);
  }
  .sight svg {
    vertical-align: -2px;
    margin-right: 4px;
    fill: currentColor;
  }
  .sight .cut {
    fill: none;
    stroke: var(--panel);
    stroke-width: 1.6;
  }
  .sight.warned {
    color: var(--warn);
  }
  .sight.fog {
    color: var(--threat);
  }
</style>
