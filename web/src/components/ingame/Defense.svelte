<script lang="ts">
  // My effective HP split next to the enemy damage mix, plus estimated enemy resists
  // (visible level + items + base stats) and the penetration notes they lead to.
  import type { Defense } from '../../lib/bindings/Defense';
  import Icon from '../Icon.svelte';

  let { d }: { d: Defense } = $props();
  const physical = $derived(Math.max(0, 1 - d.enemy_magic - d.enemy_true));
  const pct = (x: number) => `${Math.round(x * 100)}%`;
  const ehp = (x: number | null) => (x == null ? '—' : Math.round(x).toLocaleString('en-US'));
  // The side the enemy hits harder is the one to look at.
  const weakSide = $derived(
    d.ehp_magic == null || d.ehp_physical == null
      ? null
      : d.enemy_magic > physical
        ? d.ehp_magic < d.ehp_physical
          ? 'magic'
          : null
        : d.ehp_physical < d.ehp_magic
          ? 'physical'
          : null,
  );
</script>

<section class="panel" aria-label="Defense and penetration">
  <div class="top">
    <div class="ehp">
      <h2>Effective HP</h2>
      <div class="vals num">
        <span class:bad={weakSide === 'physical'}><small>vs physical</small> {ehp(d.ehp_physical)}</span>
        <span class:bad={weakSide === 'magic'}><small>vs magic</small> {ehp(d.ehp_magic)}</span>
      </div>
    </div>
    <div class="mix">
      <h2>Enemy damage</h2>
      <div class="bar" title="Threat-weighted damage mix">
        <span class="phys" style:width={pct(physical)}></span>
        <span class="mag" style:width={pct(d.enemy_magic)}></span>
        <span class="tru" style:width={pct(d.enemy_true)}></span>
      </div>
      <div class="legend num">
        <span class="phys">{pct(physical)} physical</span>
        <span class="mag">{pct(d.enemy_magic)} magic</span>
        {#if d.enemy_true >= 0.01}<span class="tru">{pct(d.enemy_true)} true</span>{/if}
      </div>
    </div>
  </div>

  {#if d.pen_notes.length}
    <ul class="notes">
      {#each d.pen_notes as n (n)}<li>{n}</li>{/each}
    </ul>
  {/if}

  {#if d.enemies.length}
    <ul class="res num" aria-label="Estimated enemy resists">
      {#each d.enemies as e (e.champ)}
        <li title="Level {e.level}: base + growth + items">
          <Icon kind="champ" id={e.champ} size={24} />
          <span>{Math.round(e.armor)}<small> AR</small></span>
          <span>{Math.round(e.mr)}<small> MR</small></span>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .top {
    display: flex;
    flex-wrap: wrap;
    gap: 10px 36px;
  }
  .vals {
    display: flex;
    gap: 22px;
    font-size: 28px;
    font-weight: 600;
    line-height: 1.1;
  }
  .vals small {
    font: 500 15px var(--sans);
    color: var(--muted);
  }
  .mix {
    flex: 1;
    min-width: 200px;
  }
  .bar {
    display: flex;
    height: 10px;
    border-radius: 5px;
    overflow: hidden;
    background: var(--raised);
  }
  .bar span {
    height: 100%;
  }
  .phys {
    --c: #e0a24a;
  }
  .mag {
    --c: #7aa6e8;
  }
  .tru {
    --c: #ebe6d9;
  }
  .bar span {
    background: var(--c);
  }
  .legend {
    display: flex;
    gap: 14px;
    margin-top: 4px;
    font-size: 16px;
  }
  .legend span {
    color: var(--c);
  }
  .notes {
    margin: 10px 0 0;
    padding: 0;
    list-style: none;
    font-size: 16px;
  }
  .notes li::before {
    content: '› ';
    color: var(--gold);
  }
  .res {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
    margin: 10px 0 0;
    padding: 0;
    list-style: none;
    font-size: 16px;
    color: var(--muted);
  }
  .res li {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .res small {
    font-size: 12px;
    color: var(--faint);
  }
</style>
