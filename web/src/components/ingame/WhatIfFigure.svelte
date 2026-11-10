<script lang="ts">
  import type { WhatIf } from '../../lib/bindings/WhatIf';
  import { store } from '../../lib/store.svelte';
  import { champName } from '../../lib/ui';
  import { ehpLine, figure, gainText } from '../../lib/whatif';

  let { w }: { w: WhatIf | undefined } = $props();
  const text = $derived(figure(w));
</script>

{#if w && text}
  <span class="whatif" tabindex="0" role="button" aria-label="What this item changes: {text}">
    <span class="fig num">{text}</span>
    <span class="pop" role="tooltip">
      {#if w.damage >= 0.005}
        <table>
          <thead>
            <tr><th></th><th class:lead={w.burst_leads}>Burst</th><th class:lead={!w.burst_leads}>10 s</th></tr>
          </thead>
          <tbody>
            {#each w.per_enemy as e (e.champ)}
              <tr class:laner={e.lane}>
                <td>{store.ui ? champName(store.ui, e.champ) : e.champ}{e.lane ? ' (lane)' : ''}</td>
                <td class="num">{gainText(e.burst)}</td>
                <td class="num">{gainText(e.sustained)}</td>
              </tr>
            {/each}
            <tr class="all"><td>Weighted</td><td class="num">{gainText(w.burst)}</td><td class="num">{gainText(w.sustained)}</td></tr>
          </tbody>
        </table>
      {/if}
      {#if ehpLine(w)}<span class="line">{ehpLine(w)}</span>{/if}
      {#if w.rough}<span class="line muted">Rough: class profile, no kit for this champion</span>{/if}
      {#if w.note}<span class="line muted">{w.note}</span>{/if}
      <span class="line muted">Vs their estimated resists, laner and fed enemies weigh most</span>
    </span>
  </span>
{/if}

<style>
  .whatif {
    position: relative;
    display: inline-flex;
    cursor: help;
    outline: none;
  }
  .fig {
    padding: 1px 7px;
    border-radius: 999px;
    background: rgb(255 255 255 / 0.06);
    color: var(--text);
    font-size: 14px;
    white-space: nowrap;
  }
  .pop {
    display: none;
    position: absolute;
    right: 0;
    top: calc(100% + 6px);
    z-index: 20;
    min-width: 260px;
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: var(--r);
    background: var(--panel);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.45);
    font-size: 14px;
    white-space: normal;
  }
  .whatif:hover .pop,
  .whatif:focus-within .pop {
    display: grid;
    gap: 6px;
  }
  table {
    border-collapse: collapse;
    width: 100%;
  }
  th {
    font-weight: 500;
    color: var(--faint);
    text-align: right;
  }
  th.lead {
    color: var(--gold);
  }
  td {
    padding: 1px 0;
  }
  td.num {
    text-align: right;
    padding-left: 12px;
  }
  tr.laner td:first-child {
    font-weight: 700;
  }
  tr.all td {
    border-top: 1px solid var(--line);
    padding-top: 3px;
    color: var(--muted);
  }
  .line {
    line-height: 1.3;
  }
</style>
