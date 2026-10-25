<script lang="ts">
  // My units: recognised on my board and bench (or tracked from purchases; ★ from copies,
  // copies toward the next star), pieces not recognised yet, and the units of my target comp
  // other players hold (copies seen on their boards, copies left in the pool).
  import type { Contest } from '../../lib/tft-bindings/Contest';
  import type { OwnedUnit } from '../../lib/tft-bindings/OwnedUnit';
  import { tft } from '../../lib/tft-store.svelte';
  import { stars, tftName, unitCost } from '../../lib/tft';
  import TftIcon from '../TftIcon.svelte';

  let { owned, contested }: { owned: OwnedUnit[]; contested: Contest[] } = $props();

  const sorted = $derived(
    owned
      .filter((u) => u.api)
      .sort((a, b) => b.star - a.star || unitCost(tft.set, b.api) - unitCost(tft.set, a.api) || b.copies - a.copies),
  );
  /** Pieces on screen nobody could name yet. */
  const unknown = $derived(owned.filter((u) => !u.api));
  const unknownBoard = $derived(unknown.reduce((n, u) => n + u.board, 0));
  const unknownBench = $derived(unknown.reduce((n, u) => n + u.bench, 0));
  /** Copies needed for the next star: 3 for ★2, 9 for ★3. */
  const next = (u: OwnedUnit) => (u.star >= 3 ? null : u.star === 2 ? 9 : 3);
</script>

<section class="panel" aria-label="My units">
  <h2>My units</h2>
  {#if sorted.length}
    <ul class="owned">
      {#each sorted as u (u.api)}
        {@const n = next(u)}
        <li class:benched={u.bench > 0 && u.board === 0}>
          <TftIcon kind="unit" api={u.api} size={40} />
          <span class="stars s{u.star}" aria-label="{u.star} star">{stars(u.star)}</span>
          <span class="name">{tftName(tft.set, 'unit', u.api)}</span>
          <span class="copies muted">
            {#if u.bench > 0}<span class="bench">{u.board > 0 ? `+${u.bench} bench` : 'bench'}</span>{/if}
            {#if n}<span class="num" title="Copies toward the next star">{u.copies}/{n}</span>{/if}
          </span>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="empty">No units tracked yet.</p>
  {/if}
  {#if unknownBoard + unknownBench}
    <p class="unknown muted">
      Not recognised yet:
      {[unknownBoard && `${unknownBoard} on the board`, unknownBench && `${unknownBench} on the bench`].filter(Boolean).join(', ')}
    </p>
  {/if}

  {#if contested.length}
    <h3>Contested</h3>
    <ul class="contested">
      {#each contested as c (c.unit)}
        <li title="{c.boards} other board{c.boards > 1 ? 's' : ''}">
          <TftIcon kind="unit" api={c.unit} size={28} />
          <span class="name">{tftName(tft.set, 'unit', c.unit)}</span>
          {#if c.held}
            <span class="boards num">{c.held} held</span>
            {#if c.left !== null}<span class="left num" class:low={c.left <= 3}>{c.left} left</span>{/if}
          {:else}
            <span class="boards num">{c.boards} board{c.boards > 1 ? 's' : ''}</span>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .owned {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 6px;
  }
  .owned li {
    display: grid;
    grid-template-columns: auto 1fr auto;
    grid-template-rows: auto auto;
    column-gap: 8px;
    align-items: center;
    padding: 4px 8px 4px 4px;
    border-radius: var(--r);
    background: var(--raised);
  }
  .owned li :global(.icon) {
    grid-row: 1 / 3;
  }
  .stars {
    font-size: 13px;
    line-height: 1;
    color: var(--muted);
    letter-spacing: 1px;
  }
  .stars.s2 {
    color: #c9d1db;
  }
  .stars.s3 {
    color: var(--gold);
  }
  .owned .name {
    grid-column: 2 / 4;
    font: 600 16px/1.1 var(--cond);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .copies {
    grid-row: 1;
    grid-column: 3;
    font-size: 13px;
  }
  .owned li.benched {
    opacity: 0.75;
  }
  .bench {
    margin-right: 6px;
    font-size: 12px;
  }
  .unknown {
    margin: 8px 0 0;
    font-size: 14px;
  }
  h3 {
    margin: 14px 0 6px;
    font: 600 14px var(--cond);
    color: var(--threat);
  }
  .contested {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 14px;
  }
  .contested li {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .contested .name {
    font: 600 16px var(--cond);
  }
  .boards {
    color: var(--threat);
    font-size: 15px;
  }
  .left {
    color: var(--muted);
    font-size: 15px;
  }
  .left.low {
    color: var(--threat);
  }
</style>
