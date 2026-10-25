<script lang="ts">
  // Champ select: my lane opponent(s) with tips (bot lane: both of theirs), synergy with my
  // bot lane partner, and the matchup page against my direct opponent (runes, summoners,
  // skill order, starting and first items).
  import type { MatchupPlan } from '../../lib/bindings/MatchupPlan';
  import { store } from '../../lib/store.svelte';
  import { LANE_NAMES, SUMMONERS, champName, runeIcon, signed, summonerIcon } from '../../lib/ui';
  import Icon from '../Icon.svelte';
  import MatchupCard from '../MatchupCard.svelte';

  let { plan }: { plan: MatchupPlan } = $props();

  const name = (c: number) => (store.ui ? champName(store.ui, c) : '');
  const names = (cs: number[]) => cs.map(name).join(' + ');
  const games = (n: number) => (n >= 1000 ? `${(n / 1000).toFixed(1)}k` : `${n}`);
  const enemies = $derived(plan.cards.map((c) => c.champ));
</script>

<section class="panel">
  <h2>{LANE_NAMES[plan.lane]}: {names(plan.allies)}{enemies.length ? ` vs ${names(enemies)}` : ''}</h2>
  {#if !plan.cards.length}
    <p class="empty">Waiting for their {plan.lane === 'bottom' || plan.lane === 'support' ? 'bot lane' : 'laner'}…</p>
  {:else}
    <div class="cards">
      {#each plan.cards as c (c.champ)}<MatchupCard card={c} me={plan.allies[0]} />{/each}
    </div>
  {/if}
  {#if plan.partner}
    {@const p = plan.partner}
    <p class="line">
      With {name(p.champ)}:
      <span class="num" class:good={p.delta > 0} class:bad={p.delta < 0}>{signed(p.delta, (v) => v.toFixed(1))} pts</span>
      <span class="muted small">{games(p.games)} games</span>
    </p>
  {/if}
  {#if plan.build}
    {@const b = plan.build}
    <div class="build">
      <h3>Vs {name(b.vs)} <span class="num muted">· {b.win_rate.toFixed(1)}% over {games(b.games)} games</span></h3>
      <div class="row">
        {#each plan.runes as r, i (`${i}-${r.id}`)}<img src={runeIcon(r.icon)} alt={r.name} title={r.name} width={i === 0 ? 34 : 24} height={i === 0 ? 34 : 24} />{/each}
        {#if plan.runes.length}<span class="keystone">{plan.runes[0].name}</span>{/if}
        {#each b.summoners as s (s)}
          {#if store.ui && SUMMONERS[s]}<img class="sum" src={summonerIcon(store.ui, s)} alt={SUMMONERS[s][1]} title={SUMMONERS[s][1]} width="26" height="26" />{/if}
        {/each}
        {#if b.skills}<span class="num skills" title="Skill max order">{b.skills.split('').join(' › ')}</span>{/if}
      </div>
      <div class="row">
        <span class="muted">Start</span>
        {#each b.start as id, i (`${id}-${i}`)}<Icon kind="item" {id} size={28} />{/each}
        <span class="muted small">{b.start_wr.toFixed(1)}% · {games(b.start_games)}</span>
        {#if b.core.length}
          <span class="muted">then</span>
          {#each b.core as id (id)}<Icon kind="item" {id} size={28} />{/each}
        {/if}
      </div>
    </div>
  {/if}
</section>

<style>
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
    gap: 14px 20px;
  }
  .small {
    font-size: 13px;
  }
  .line {
    margin: 10px 0 0;
  }
  .build {
    margin-top: 12px;
    padding-top: 10px;
    border-top: 1px solid var(--line);
    display: grid;
    gap: 6px;
  }
  h3 {
    margin: 0;
    font: 600 15px var(--cond);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .row img {
    border-radius: 50%;
  }
  .row img.sum {
    border-radius: 4px;
  }
  .keystone {
    margin-right: 6px;
    font: 600 14px var(--cond);
  }
  .skills {
    margin-left: 6px;
    font-size: 15px;
    font-weight: 600;
  }
</style>
