<script lang="ts">
  // Death review (recap): each death with killer, gold, what Buy next said, and enemy level spikes.
  import type { DeathReview } from '../../lib/bindings/DeathReview';
  import { store } from '../../lib/store.svelte';
  import { champName, clockText, gold, signed } from '../../lib/ui';
  import Icon from '../Icon.svelte';

  let { review }: { review: DeathReview } = $props();
  const HOLDING = 1000;
  const name = (c: number) => (store.ui ? champName(store.ui, c) : '');
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
        <tr><th>Time</th><th>By</th><th>Gold vs killer</th><th>Held</th><th>Buy next</th></tr>
      </thead>
      <tbody>
        {#each review.deaths as d, i (i)}
          <tr>
            <td class="num">{clockText(d.time)}</td>
            <td class="by">
              {#if d.killer != null}
                <Icon kind="champ" id={d.killer} size={26} />
              {:else}
                <span class="muted">Executed</span>
              {/if}
              {#each d.assisters as a, j (`${j}-${a}`)}<span class="assist"><Icon kind="champ" id={a} size={18} /></span>{/each}
              {#if d.traded}<span class="tag trade" title="Your team won the fight around this death">trade</span>{/if}
              {#if d.killer_top_threat}<span class="tag">top threat</span>{/if}
              {#each d.spikes as [c, lvl], j (`${j}-${c}-${lvl}`)}
                <span class="tag spike" title="{name(c)} reached level {lvl} within 60 s before">{name(c)} {lvl}</span>
              {/each}
            </td>
            <td class="num" class:bad={!d.traded && d.gold_gap != null && d.gold_gap < 0}>{d.gold_gap != null ? signed(d.gold_gap, gold) : ''}</td>
            <td class="num" class:warn={d.unspent >= HOLDING}>{gold(d.unspent)}</td>
            <td>
              {#if d.buy_next != null}
                <span class="buy" class:afford={d.could_buy} title={d.could_buy ? 'You could afford it (or its component)' : ''}>
                  <Icon kind="item" id={d.buy_next} size={24} />
                </span>
              {/if}
            </td>
          </tr>
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
</style>
