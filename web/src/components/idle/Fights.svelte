<script lang="ts">
  // Fights in a reviewed game: result, place, my part, and the objective that followed.
  import type { Fight } from '../../lib/bindings/Fight';
  import { clockText } from '../../lib/ui';

  let { fights }: { fights: Fight[] } = $props();

  const PART: Record<Fight['presence'], string> = { there: '', dead: 'you were dead', away: 'you were elsewhere' };
  const there = $derived(fights.filter((f) => f.presence === 'there' && f.won + f.lost >= 2));
  const won = $derived(there.filter((f) => f.won > f.lost).length);
</script>

<section class="panel">
  <div class="top">
    <h2>Fights</h2>
    {#if there.length}<span class="muted small">won {won} of {there.length} you were in</span>{/if}
  </div>
  {#if !fights.length}
    <p class="empty">No fights recorded.</p>
  {:else}
    <table>
      <tbody>
        {#each fights as f, i (i)}
          <tr class:faded={f.presence !== 'there'}>
            <td class="num t">{clockText(f.start)}</td>
            <td class="num score">
              <span class="good">{f.won}</span><span class="muted">–</span><span class="bad">{f.lost}</span>
            </td>
            <td class="muted">{f.place ?? ''}</td>
            <td>
              {#if f.presence !== 'there'}<span class="tag away">{PART[f.presence]}</span>{/if}
              {#if f.died_first}<span class="tag first" title="Your death was your team's first in this fight">died first</span>{/if}
              {#if f.then}
                <span class="tag" class:good={f.then.ours} class:bad={!f.then.ours}>then {f.then.ours ? 'took' : 'lost'} {f.then.kind}</span>
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</section>

<style>
  .top {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
  }
  .small {
    font-size: 13px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 14px;
  }
  td {
    padding: 2px 8px 2px 0;
    white-space: nowrap;
  }
  .t {
    color: var(--muted);
    width: 44px;
  }
  .score {
    width: 40px;
    font-size: 16px;
  }
  .faded td {
    opacity: 0.6;
  }
  .tag {
    padding: 0 6px;
    border-radius: 999px;
    background: var(--raised);
    font-size: 12px;
    color: var(--muted);
  }
  .tag.away {
    color: var(--warn);
  }
  .tag.first {
    color: var(--threat);
  }
  .tag.good {
    color: var(--good);
  }
  .tag.bad {
    color: var(--threat);
  }
</style>
