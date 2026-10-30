<script lang="ts">
  // Where I die on the Rift across recent games, per champion and game phase.
  import type { PastGame } from '../../lib/bindings/PastGame';
  import { store } from '../../lib/store.svelte';
  import { champName, clockText, zoneOf } from '../../lib/ui';
  import DeathMap, { type Spot } from './DeathMap.svelte';

  let { games }: { games: PastGame[] } = $props();

  const EARLY = 14 * 60;
  let champ = $state<number | 'all'>('all');
  let phase = $state<'all' | 'early' | 'late'>('all');

  const rift = $derived(games.filter((g) => g.mode === 'rift' && !g.remake && g.death_times != null));
  const champs = $derived([...new Set(rift.map((g) => g.champ))]);
  const picked = $derived(rift.filter((g) => champ === 'all' || g.champ === champ));
  const deaths = $derived(
    picked.flatMap((g) =>
      g.death_spots
        .filter((d) => phase === 'all' || (phase === 'early') === d.time < EARLY)
        .map((d) => ({ ...d, win: g.win, champ: g.champ, zone: zoneOf(d.x, d.y) })),
    ),
  );
  const name = (c: number) => (store.ui ? champName(store.ui, c) : '');
  const spots = $derived<Spot[]>(
    deaths.map((d) => ({ x: d.x, y: d.y, tone: d.win ? 'win' : 'loss', title: `${name(d.champ)} ${clockText(d.time)} · ${d.zone}` })),
  );
  // Most common places, losses and wins apart.
  const places = $derived.by(() => {
    const m = new Map<string, [number, number]>();
    for (const d of deaths) {
      const z = d.zone;
      const c = m.get(z) ?? [0, 0];
      c[d.win ? 1 : 0]++;
      m.set(z, c);
    }
    return [...m].sort((a, b) => b[1][0] + b[1][1] - (a[1][0] + a[1][1])).slice(0, 5);
  });
</script>

<section class="panel">
  <h2>Where you die</h2>
  {#if !rift.length}
    <p class="empty">Rift games with a timeline show your deaths here.</p>
  {:else}
    <div class="filters">
      <select bind:value={champ} aria-label="Champion">
        <option value="all">All champions</option>
        {#each champs as c (c)}<option value={c}>{name(c)}</option>{/each}
      </select>
      <select bind:value={phase} aria-label="Game phase">
        <option value="all">Whole game</option>
        <option value="early">Before 14:00</option>
        <option value="late">After 14:00</option>
      </select>
      <span class="muted small">{deaths.length} deaths in {picked.length} games</span>
    </div>
    <div class="body">
      <DeathMap {spots} size={240} />
      <div class="side">
        <p class="small legend"><i class="dot loss"></i>in losses <i class="dot win"></i>in wins</p>
        {#if places.length}
          <table>
            <tbody>
              {#each places as [z, [l, w]] (z)}
                <tr>
                  <th>{z}</th>
                  <td class="num bad">{l}</td>
                  <td class="num good">{w}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}
      </div>
    </div>
  {/if}
</section>

<style>
  .filters {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-bottom: 10px;
  }
  select {
    padding: 3px 6px;
    border: 1px solid var(--line);
    border-radius: 4px;
    background: var(--raised);
  }
  .small {
    font-size: 13px;
  }
  .body {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
    align-items: flex-start;
  }
  .side {
    flex: 1;
    min-width: 150px;
  }
  .legend {
    margin: 0 0 8px;
    color: var(--muted);
  }
  .dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    margin: 0 4px 0 6px;
    border-radius: 50%;
    background: var(--threat);
  }
  .dot.win {
    background: var(--good);
  }
  table {
    border-collapse: collapse;
    width: 100%;
  }
  th {
    text-align: left;
    font-weight: 400;
    color: var(--muted);
  }
  td {
    text-align: right;
    padding-left: 10px;
    font-size: 16px;
  }
</style>
