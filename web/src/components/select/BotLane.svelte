<script lang="ts">
  // Bot lane 2v2 plan (me support or ADC): my win rate against each of their bot laners,
  // their lane windows from the notes, my past games against them, synergy with my
  // partner, and starting items for the matchup.
  import type { BotLanePlan } from '../../lib/bindings/BotLanePlan';
  import { store } from '../../lib/store.svelte';
  import { champName, gold, signed } from '../../lib/ui';
  import Icon from '../Icon.svelte';

  let { plan }: { plan: BotLanePlan } = $props();

  /** Matchup rows from fewer games are shown greyed. */
  const FEW = 100;
  const name = (c: number) => (store.ui ? champName(store.ui, c) : '');
  const names = (cs: number[]) => cs.map(name).join(' + ');
  const games = (n: number) => (n >= 1000 ? `${(n / 1000).toFixed(1)}k` : `${n}`);
  const vs = (c: number) => plan.numbers.find((n) => n.champ === c && !n.with);
  const window = (c: number) => plan.windows.find((w) => w.champ === c);
  const mem = (c: number) => plan.memory.find((m) => m.champ === c);
  const partner = $derived(plan.numbers.find((n) => n.with));
</script>

<section class="panel">
  <h2>Bot lane: {names(plan.allies)}{plan.enemies.length ? ` vs ${names(plan.enemies)}` : ''}</h2>
  {#if !plan.enemies.length}
    <p class="empty">Waiting for their bot lane…</p>
  {:else}
    <ul>
      {#each plan.enemies as e (e)}
        {@const n = vs(e)}
        {@const w = window(e)}
        {@const m = mem(e)}
        <li>
          <Icon kind="champ" id={e} size={40} />
          <div class="body">
            <div class="head">
              <span class="who">{name(e)}</span>
              {#if n}
                <span class="num" class:few={n.games < FEW} class:good={n.games >= FEW && n.value >= 51} class:bad={n.games >= FEW && n.value <= 49}
                  >{n.value.toFixed(1)}%</span
                >
                <span class="muted small">{games(n.games)} games{n.games < FEW ? ', few' : ''}</span>
              {/if}
              {#each w?.tags ?? [] as t (t)}<span class="tag">{t}</span>{/each}
            </div>
            {#if w}<p class="note">{w.note}</p>{/if}
            {#if m}
              <p class="muted small">
                You: <span class="good">{m.wins}W</span> <span class="bad">{m.games - m.wins}L</span>{#if m.gold_15 != null}, {signed(m.gold_15, gold)} at 15{/if}{#if m.killed_me}, killed you {m.killed_me}×{/if}
              </p>
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  {/if}
  {#if partner}
    <p class="line">
      With {name(partner.champ)}:
      <span class="num" class:good={partner.value > 0} class:bad={partner.value < 0}>{signed(partner.value, (v) => v.toFixed(1))} pts</span>
      <span class="muted small">{games(partner.games)} games</span>
    </p>
  {/if}
  {#if plan.start}
    {@const s = plan.start}
    <div class="items">
      <span class="muted">Start vs {name(s.vs)}</span>
      {#each s.start as id, i (`${id}-${i}`)}<Icon kind="item" {id} size={30} />{/each}
      <span class="muted small">{s.start_wr.toFixed(1)}% · {games(s.start_games)}</span>
      {#if s.core.length}
        <span class="muted">then</span>
        {#each s.core as id (id)}<Icon kind="item" {id} size={30} />{/each}
      {/if}
    </div>
  {/if}
</section>

<style>
  ul {
    display: grid;
    gap: 10px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    align-items: flex-start;
    gap: 10px;
  }
  .body {
    display: grid;
    gap: 2px;
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 8px;
  }
  .who {
    font: 600 17px var(--cond);
  }
  .few {
    color: var(--faint);
  }
  .tag {
    padding: 0 7px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--warn) 20%, var(--raised));
    font-size: 12px;
  }
  .note {
    margin: 0;
    font-size: 14px;
  }
  .small {
    font-size: 13px;
  }
  p.small {
    margin: 0;
  }
  .line {
    margin: 10px 0 0;
  }
  .items {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-top: 10px;
  }
</style>
