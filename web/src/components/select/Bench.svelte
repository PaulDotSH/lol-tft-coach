<script lang="ts">
  // ARAM / Mayhem: your champion and the bench as options for this team, each with
  // its tier or ARAM win rate and what it adds to the team. Options, not an order to swap.
  import type { BenchRank } from '../../lib/bindings/BenchRank';
  import { store } from '../../lib/store.svelte';
  import { champName } from '../../lib/ui';
  import Icon from '../Icon.svelte';
  import { pts, tone } from './draft';

  let { bench }: { bench: BenchRank[] } = $props();
  const tierClass = (t: string | null) => (t ? (t.startsWith('S') ? 's' : t.startsWith('A') ? 'a' : t.startsWith('B') ? 'b' : 'c') : 'c');
  const mine = $derived(bench.find((b) => b.mine));
</script>

<section class="panel">
  <h2>Bench options for this team</h2>
  <ol>
    {#each bench as b (b.champ)}
      <li class:mine={b.mine}>
        <span class="pic">
          <Icon kind="champ" id={b.champ} size={44} />
          <span class="tier num {tierClass(b.tier)}">{b.tier ?? '?'}</span>
        </span>
        <div class="text">
          <div class="name">
            {store.ui ? champName(store.ui, b.champ) : ''}
            {#if b.mine}<span class="you">you</span>{/if}
          </div>
          <div class="reasons muted">{b.reasons.length ? b.reasons.join(' · ') : 'No tier or win rate yet'}</div>
        </div>
        {#if b.score != null}
          <span
            class="num score {b.mine ? 'muted' : tone(b.score - (mine?.score ?? b.score), 0.25)}"
            title={b.mine ? 'Tier / win rate + team fit, win-rate points' : 'vs your champion, win-rate points'}
            >{b.mine || mine?.score == null ? pts(b.score) : pts(b.score - mine.score)}</span
          >
        {/if}
      </li>
    {/each}
  </ol>
  <p class="note">Tier from u.gg (Mayhem) or ARAM win rate from lolalytics, plus what each fills in your team.</p>
</section>

<style>
  ol {
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    gap: 6px;
  }
  li {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    padding: 4px 8px 4px 4px;
    border-radius: var(--r);
  }
  .mine {
    background: var(--raised);
  }
  .mine :global(.icon) {
    box-shadow: 0 0 0 2px var(--text);
  }
  .pic {
    position: relative;
  }
  .tier {
    position: absolute;
    right: -6px;
    bottom: -4px;
    min-width: 22px;
    padding: 0 3px;
    border-radius: 4px;
    font-size: 13px;
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
    font: 600 19px/1.1 var(--cond);
  }
  .you {
    margin-left: 6px;
    font: 500 12px/1 var(--sans);
    color: var(--muted);
  }
  .reasons {
    font-size: 13px;
  }
  .score {
    font-size: 18px;
    font-weight: 600;
  }
  .note {
    margin: 8px 0 0;
    font-size: 12px;
    color: var(--faint);
  }
</style>
