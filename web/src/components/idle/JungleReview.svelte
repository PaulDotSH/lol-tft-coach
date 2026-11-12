<script lang="ts">
  // Jungle review in place of laning: first clear, clear rate, ganks that got a kill, objectives, invades,
  // and me against their jungler.
  import type { JungleReview } from '../../lib/bindings/JungleReview';
  import { store } from '../../lib/store.svelte';
  import { champName, clockText, gold, LANE_NAMES, signed } from '../../lib/ui';
  import Icon from '../Icon.svelte';

  let { j, rich }: { j: JungleReview; rich: number[] } = $props();
  const name = (c: number) => (store.ui ? champName(store.ui, c) : '');
  const vs = (mine: number, theirs: number | null) => (theirs == null ? 0 : mine - theirs);
  const PRESENCE: Record<string, string> = { there: 'there', dead: 'dead', away: 'away' };
</script>

<section class="panel">
  <h2>Jungle{j.enemy != null ? ` vs ${name(j.enemy)}` : ''}</h2>
  {#if j.first_clear.length}
    <p class="lead">
      {#each j.first_clear as c, i (c.minute)}{i ? ' · ' : ''}Level {c.level}, {c.jungle_cs} jungle CS at {c.minute}:00{/each}
    </p>
  {/if}

  {#if j.clear.length}
    <h3>Clear rate <span class="muted">(jungle CS per minute; all CS{j.benchmark != null ? ` vs ${j.benchmark.toFixed(1)} average` : ''})</span></h3>
    <table>
      <tbody>
        {#each j.clear as c (c.from)}
          <tr>
            <th class="num">{c.from}–{c.to} min</th>
            <td class="num">{c.jungle_per_min.toFixed(1)}</td>
            <td class="num" class:good={j.benchmark != null && c.cs_per_min >= j.benchmark} class:bad={j.benchmark != null && c.cs_per_min < j.benchmark * 0.85}>
              {c.cs_per_min.toFixed(1)}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}

  <h3>Ganks that got a kill <span class="muted">(before 14:00; attempts without a kill aren't in the timeline)</span></h3>
  {#if j.ganks.length}
    <ul>
      {#each j.ganks as g (g.lane)}
        <li class:good={g.kills > g.deaths} class:bad={g.deaths > g.kills}>
          {LANE_NAMES[g.lane]}: {g.ganks} gank{g.ganks === 1 ? '' : 's'}, {g.kills} kill{g.kills === 1 ? '' : 's'}, {g.deaths} death{g.deaths === 1 ? '' : 's'}
        </li>
      {/each}
    </ul>
  {:else}
    <p class="muted small">None.</p>
  {/if}

  {#if j.objectives.length}
    <h3>Objectives</h3>
    <ul>
      {#each j.objectives as o (o.time)}
        <li class:good={o.ours} class:bad={!o.ours}>
          <span class="num muted">{clockText(o.time)}</span>
          <span>
            {o.kind}{o.detail ? ` ${o.detail}` : ''} · {o.ours ? 'yours' : 'theirs'}{o.presence ? ` · you ${PRESENCE[o.presence]}` : ''}{o.enemy_jungler_dead
              ? ' · their jungler dead'
              : ''}
          </span>
        </li>
      {/each}
    </ul>
  {/if}

  {#if j.invades.length}
    <p class="small">Camps in their jungle at {j.invades.map((m) => `${m}:00`).join(', ')}.</p>
  {/if}

  {#if j.versus.length}
    <h3>
      {#if j.enemy != null}<Icon kind="champ" id={j.enemy} size={18} />{/if} Against their jungler <span class="muted">(jungle CS, level, gold)</span>
    </h3>
    <table>
      <tbody>
        {#each j.versus as r (r.minute)}
          <tr>
            <th class="num">{r.minute}:00</th>
            <td class="num">
              <span class:good={vs(r.cs, r.opp_cs) > 0} class:bad={vs(r.cs, r.opp_cs) < 0}>{r.cs}</span>{#if r.opp_cs != null}<span class="muted"> / {r.opp_cs}</span>{/if}
            </td>
            <td class="num">
              <span class:good={vs(r.level, r.opp_level) > 0} class:bad={vs(r.level, r.opp_level) < 0}>{r.level}</span>{#if r.opp_level != null}<span class="muted"> / {r.opp_level}</span>{/if}
            </td>
            <td class="num" class:good={(r.gold_lead ?? 0) > 0} class:bad={(r.gold_lead ?? 0) < 0}>{r.gold_lead != null ? signed(r.gold_lead, gold) : ''}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
  {#if rich.length}
    <p class="muted small" class:warn={rich.length >= 2}>You held 1500+ gold unspent at {rich.map((m) => `${m}:00`).join(', ')}.</p>
  {/if}
</section>

<style>
  .lead {
    margin: 0 0 8px;
    font-size: 16px;
    font-weight: 600;
  }
  h3 {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 12px 0 4px;
    font-size: 13px;
    font-weight: 500;
    color: var(--muted);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 14px;
  }
  th {
    text-align: left;
    font-weight: 400;
    color: var(--muted);
  }
  td {
    text-align: right;
    padding: 1px 0 1px 8px;
    font-variant-numeric: tabular-nums;
  }
  ul {
    display: grid;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: 14px;
  }
  li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding-left: 8px;
    border-left: 3px solid var(--line);
  }
  li.good {
    border-left-color: var(--good);
  }
  li.bad {
    border-left-color: var(--threat);
  }
  .small {
    margin: 8px 0 0;
    font-size: 13px;
  }
  .warn {
    color: var(--warn);
  }
</style>
