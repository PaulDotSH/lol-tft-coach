<script lang="ts">
  import { api } from '../../lib/api';
  import type { ChampStats } from '../../lib/bindings/ChampStats';
  import type { ItemStat } from '../../lib/bindings/ItemStat';
  import type { Lane } from '../../lib/bindings/Lane';
  import type { Mode } from '../../lib/bindings/Mode';
  import { store } from '../../lib/store.svelte';
  import { champName, LANE_NAMES, LANES, MODE_NAMES, pct } from '../../lib/ui';
  import Icon from '../Icon.svelte';

  let query = $state('');
  let champ = $state<number | null>(null);
  let lane = $state<Lane | null>(null);
  let mode = $state<Mode>('rift');
  let stats = $state.raw<ChampStats | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);

  const champs = $derived(
    store.ui
      ? Object.entries(store.ui.champs)
          .map(([k, c]) => ({ key: Number(k), name: c.name }))
          .sort((a, b) => a.name.localeCompare(b.name))
      : [],
  );
  const matches = $derived.by(() => {
    const q = query.trim().toLowerCase().replace(/[^a-z]/g, '');
    if (!q || (champ != null && store.ui && champName(store.ui, champ) === query)) return [];
    return champs.filter((c) => c.name.toLowerCase().replace(/[^a-z]/g, '').includes(q)).slice(0, 8);
  });

  let seq = 0;
  async function run() {
    if (champ == null) return;
    const id = ++seq;
    loading = true;
    error = null;
    try {
      const r = await (await api).lookup(champ, mode === 'rift' ? lane : null, mode);
      if (id === seq) stats = r;
    } catch (e) {
      if (id === seq) error = String(e);
    } finally {
      if (id === seq) loading = false;
    }
  }
  function choose(key: number, name: string) {
    champ = key;
    query = name;
    run();
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === 'Enter' && matches[0]) choose(matches[0].key, matches[0].name);
  }

  const wr = (s: ItemStat) => (s.games ? (100 * s.wins) / s.games : 0);
  const counters = $derived.by(() => {
    if (!stats) return { best: [], worst: [] };
    const solid = stats.counters.filter((c) => c.games >= 200).sort((a, b) => b.vs_wr - a.vs_wr);
    return { best: solid.slice(0, 5), worst: solid.slice(-5).reverse() };
  });
</script>

<section class="panel lookup">
  <h2>Champion lookup</h2>
  <div class="controls">
    <div class="search">
      <input
        type="search"
        placeholder="Search a champion"
        bind:value={query}
        onkeydown={onKey}
        aria-label="Champion"
        autocomplete="off"
      />
      {#if matches.length}
        <ul class="menu" role="listbox">
          {#each matches as m (m.key)}
            <li><button type="button" onclick={() => choose(m.key, m.name)}><Icon kind="champ" id={m.key} size={24} />{m.name}</button></li>
          {/each}
        </ul>
      {/if}
    </div>
    <div class="seg" role="group" aria-label="Mode">
      {#each Object.entries(MODE_NAMES) as [m, name] (m)}
        <button type="button" class:on={mode === m} onclick={() => ((mode = m as Mode), run())}>{name}</button>
      {/each}
    </div>
    {#if mode === 'rift'}
      <div class="seg" role="group" aria-label="Lane">
        <button type="button" class:on={lane === null} onclick={() => ((lane = null), run())}>Main</button>
        {#each LANES as l (l)}
          <button type="button" class:on={lane === l} onclick={() => ((lane = l), run())}>{LANE_NAMES[l]}</button>
        {/each}
      </div>
    {/if}
  </div>

  {#if error}
    <p class="bad">Lookup failed: {error}</p>
  {:else if !stats}
    <p class="empty">{loading ? 'Loading…' : 'Pick a champion to see its build, starting items and counters.'}</p>
  {:else}
    <div class="result" class:stale={loading}>
      <div class="summary">
        <Icon kind="champ" id={stats.champ} size={44} />
        <span class="cname">{store.ui ? champName(store.ui, stats.champ) : ''}</span>
        <span><span class="num big">{pct(stats.win_rate)}</span> <span class="muted">win</span></span>
        <span><span class="num big">{pct(stats.pick_rate)}</span> <span class="muted">pick</span></span>
        <span class="muted">{Number(stats.games).toLocaleString()} games, {stats.patch.includes('.') ? `patch ${stats.patch}` : `last ${stats.patch} days`}</span>
      </div>

      <div class="slots">
        {#each stats.slots as slot, i (i)}
          <div class="slot">
            <h2>Item {i + 1}</h2>
            {#each slot.slice(0, 3) as it (it.id)}
              <div class="istat"><Icon kind="item" id={it.id} size={34} /><span class="num">{pct(wr(it))}</span></div>
            {/each}
          </div>
        {/each}
        {#if stats.boots.length}
          <div class="slot">
            <h2>Boots</h2>
            {#each stats.boots.slice(0, 3) as it (it.id)}
              <div class="istat"><Icon kind="item" id={it.id} size={34} /><span class="num">{pct(wr(it))}</span></div>
            {/each}
          </div>
        {/if}
      </div>

      <div class="lower">
        <div>
          <h2>Starting items</h2>
          {#each stats.early.slice(0, 3) as e, i (i)}
            <div class="early">
              <span class="icons">{#each e.items as id, j (`${j}-${id}`)}<Icon kind="item" {id} size={28} />{/each}</span>
              <span class="num">{pct(e.win_rate)}</span>
              <span class="num muted">{pct(e.pick_rate, 0)} pick</span>
            </div>
          {/each}
        </div>
        {#each [['Best against', counters.best, 'good'], ['Worst against', counters.worst, 'bad']] as const as [title, list, tone] (title)}
          <div>
            <h2>{title}</h2>
            {#each list as c (c.champ)}
              <div class="counter">
                <Icon kind="champ" id={c.champ} size={24} />
                <span class="cn">{store.ui ? champName(store.ui, c.champ) : ''}</span>
                <span class="num {tone}">{pct(c.vs_wr)}</span>
              </div>
            {/each}
          </div>
        {/each}
      </div>
    </div>
  {/if}
</section>

<style>
  .controls {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-bottom: 14px;
  }
  .search {
    position: relative;
    flex: 1 1 200px;
  }
  input {
    width: 100%;
    padding: 6px 10px;
    border: 1px solid var(--line);
    border-radius: var(--r);
    background: var(--bg);
  }
  .menu {
    position: absolute;
    z-index: 5;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    margin: 0;
    padding: 4px;
    list-style: none;
    border: 1px solid var(--line);
    border-radius: var(--r);
    background: var(--raised);
  }
  .menu button {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 4px 6px;
    border: 0;
    border-radius: 4px;
    background: none;
    text-align: left;
    cursor: pointer;
  }
  .menu button:hover,
  .menu button:focus-visible {
    background: var(--line);
  }
  .seg {
    display: flex;
    border: 1px solid var(--line);
    border-radius: var(--r);
    overflow: hidden;
  }
  .seg button {
    padding: 5px 10px;
    border: 0;
    background: none;
    color: var(--muted);
    font-size: 13px;
    cursor: pointer;
  }
  .seg button + button {
    border-left: 1px solid var(--line);
  }
  .seg .on {
    background: var(--raised);
    color: var(--text);
  }
  .stale {
    opacity: 0.6;
  }
  .summary {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 18px;
    margin-bottom: 14px;
  }
  .cname {
    font: 700 24px var(--cond);
  }
  .big {
    font-size: 22px;
    font-weight: 600;
  }
  .slots {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(84px, 1fr));
    gap: 10px;
    padding-bottom: 14px;
    border-bottom: 1px solid var(--line);
  }
  .slot {
    display: grid;
    gap: 6px;
    align-content: start;
  }
  .istat {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 15px;
  }
  .lower {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(170px, 1fr));
    gap: 16px;
    margin-top: 14px;
  }
  .early,
  .counter {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 5px;
  }
  .icons {
    display: flex;
    gap: 2px;
  }
  .cn {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
