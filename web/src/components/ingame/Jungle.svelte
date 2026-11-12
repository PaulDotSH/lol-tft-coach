<script lang="ts">
  // Jungler: clear pace and the lane board (which lanes are open to a gank and why), in place of the laning panel.
  import type { JungleAdvice } from '../../lib/bindings/JungleAdvice';
  import { clockText, LANE_NAMES } from '../../lib/ui';
  import Icon from '../Icon.svelte';

  let { j }: { j: JungleAdvice } = $props();
  const quiet = (reasons: string[]) => reasons.length === 1 && reasons[0] === 'nothing special';
</script>

<section class="panel">
  <h2>Jungle {#if !j.minimap}<span class="muted note">no minimap</span>{/if}</h2>
  {#if j.pace}<p class="pace">{j.pace}</p>{/if}
  {#if j.lanes.length}
    <ol>
      {#each j.lanes as l (l.lane)}
        <li class:quiet={quiet(l.reasons)}>
          <span class="lane">{LANE_NAMES[l.lane]}</span>
          <span class="who">{#each l.enemy as c (c)}<Icon kind="champ" id={c} size={22} />{/each}</span>
          <span class="why">{l.reasons.join(' · ')}</span>
        </li>
      {/each}
    </ol>
  {/if}
  {#if j.camps.length}
    <ul class="camps">
      {#each j.camps as c (c.name)}<li><span>{c.name}</span><span class="num">{clockText(c.respawn_at)}</span></li>{/each}
    </ul>
  {/if}
</section>

<style>
  h2 {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  .note {
    font-size: 12px;
    font-weight: 400;
  }
  .pace {
    margin: 0 0 8px;
    font-size: 15px;
  }
  ol,
  .camps {
    display: grid;
    gap: 4px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  ol li {
    display: grid;
    grid-template-columns: 44px auto 1fr;
    gap: 8px;
    align-items: center;
    padding-left: 8px;
    border-left: 3px solid var(--good);
    font-size: 14px;
  }
  ol li.quiet {
    border-left-color: var(--line);
    color: var(--muted);
  }
  .lane {
    font: 700 12px var(--cond);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
  }
  .who {
    display: inline-flex;
    gap: 2px;
  }
  .camps {
    margin-top: 8px;
    font-size: 13px;
  }
  .camps li {
    display: flex;
    justify-content: space-between;
  }
</style>
