<script lang="ts">
  // My units as tracked from purchases (★ from copies; copies toward the next star), and the
  // units of my target comp seen on other boards while scouting.
  import type { OwnedUnit } from '../../lib/tft-bindings/OwnedUnit';
  import { tft } from '../../lib/tft-store.svelte';
  import { stars, tftName, unitCost } from '../../lib/tft';
  import TftIcon from '../TftIcon.svelte';

  let { owned, contested }: { owned: OwnedUnit[]; contested: [string, number][] } = $props();

  const sorted = $derived(
    [...owned].sort((a, b) => b.star - a.star || unitCost(tft.set, b.api) - unitCost(tft.set, a.api) || b.copies - a.copies),
  );
  /** Copies needed for the next star: 3 for ★2, 9 for ★3. */
  const next = (u: OwnedUnit) => (u.star >= 3 ? null : u.star === 2 ? 9 : 3);
</script>

<section class="panel" aria-label="My units">
  <h2>My units</h2>
  {#if sorted.length}
    <ul class="owned">
      {#each sorted as u (u.api)}
        {@const n = next(u)}
        <li>
          <TftIcon kind="unit" api={u.api} size={40} />
          <span class="stars s{u.star}" aria-label="{u.star} star">{stars(u.star)}</span>
          <span class="name">{tftName(tft.set, 'unit', u.api)}</span>
          {#if n}<span class="copies num muted" title="Copies toward the next star">{u.copies}/{n}</span>{/if}
        </li>
      {/each}
    </ul>
  {:else}
    <p class="empty">No units tracked yet.</p>
  {/if}

  {#if contested.length}
    <h3>Contested</h3>
    <ul class="contested">
      {#each contested as [api, boards] (api)}
        <li>
          <TftIcon kind="unit" api={api} size={28} />
          <span class="name">{tftName(tft.set, 'unit', api)}</span>
          <span class="boards num">{boards} board{boards > 1 ? 's' : ''}</span>
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
</style>
