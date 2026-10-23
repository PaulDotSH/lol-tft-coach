<script lang="ts">
  // CS, levels and gold against my lane opponent at 5, 10, 15 and 20 minutes.
  import type { PhaseRow } from '../../lib/bindings/PhaseRow';
  import { gold, signed } from '../../lib/ui';

  let { rows, opponent, rich }: { rows: PhaseRow[]; opponent: string | null; rich: number } = $props();
  const vs = (mine: number, theirs: number | null) => (theirs == null ? 0 : mine - theirs);
</script>

<section class="panel">
  <h2>By minute{opponent ? ` vs ${opponent}` : ''}</h2>
  <table>
    <thead>
      <tr><th>Minute</th><th>CS</th><th>Level</th>{#if opponent}<th>Gold lead</th>{/if}</tr>
    </thead>
    <tbody>
      {#each rows as r (r.minute)}
        <tr>
          <th class="num">{r.minute}:00</th>
          <td class="num">
            <span class:good={vs(r.cs, r.opp_cs) > 0} class:bad={vs(r.cs, r.opp_cs) < 0}>{r.cs}</span>
            {#if r.opp_cs != null}<span class="muted"> / {r.opp_cs}</span>{/if}
          </td>
          <td class="num">
            <span class:good={vs(r.level, r.opp_level) > 0} class:bad={vs(r.level, r.opp_level) < 0}>{r.level}</span>
            {#if r.opp_level != null}<span class="muted"> / {r.opp_level}</span>{/if}
          </td>
          {#if opponent}
            <td class="num" class:good={(r.gold_lead ?? 0) > 0} class:bad={(r.gold_lead ?? 0) < 0}>
              {r.gold_lead != null ? signed(r.gold_lead, gold) : ''}
            </td>
          {/if}
        </tr>
      {/each}
    </tbody>
  </table>
  {#if rich > 0}
    <p class="muted small" class:warn={rich >= 3}>You held 1500+ gold at {rich} minute mark{rich === 1 ? '' : 's'}.</p>
  {/if}
</section>

<style>
  table {
    width: 100%;
    border-collapse: collapse;
  }
  thead th {
    font: 600 12px var(--cond);
    color: var(--faint);
    text-align: right;
    padding-bottom: 4px;
  }
  thead th:first-child,
  tbody th {
    text-align: left;
  }
  tbody th {
    font-weight: 400;
    color: var(--muted);
  }
  td {
    text-align: right;
    font-size: 16px;
    padding: 1px 0 1px 12px;
  }
  .small {
    margin: 8px 0 0;
    font-size: 13px;
  }
  .warn {
    color: var(--warn);
  }
</style>
