<script lang="ts">
  // Review of one match-history game: takeaways, my numbers, gold over time,
  // deaths with places and trades, objectives, items and both teams.
  import { api } from '../../lib/api';
  import type { MatchAnalysis } from '../../lib/bindings/MatchAnalysis';
  import { store } from '../../lib/store.svelte';
  import { champName, clockText, LANE_NAMES, queueName } from '../../lib/ui';
  import Icon from '../Icon.svelte';
  import DeathMap from './DeathMap.svelte';
  import DeathReview from './DeathReview.svelte';
  import Fights from './Fights.svelte';
  import GoldChart from './GoldChart.svelte';
  import Scoreboard from './Scoreboard.svelte';

  let { gameId, onclose }: { gameId: number; onclose: () => void } = $props();

  let a = $state.raw<MatchAnalysis | null>(null);
  let error = $state<string | null>(null);
  $effect(() => {
    const id = gameId;
    a = null;
    error = null;
    api
      .then((x) => x.analyzeGame(id))
      .then((r) => id === gameId && (a = r))
      .catch((e) => id === gameId && (error = String(e)));
  });

  const name = (c: number) => (store.ui ? champName(store.ui, c) : '');
  const g = $derived(a?.game);
  const min = $derived(Math.max(1, (g?.duration_s ?? 60) / 60));
  // [label, mine per minute, champion average, digits]
  const rows = $derived<[string, number, number | null, number][]>(
    a && g
      ? [
          ['CS / min', g.cs / min, a.benchmarks?.cs_per_min ?? null, 1],
          ['Gold / min', g.gold / min, a.benchmarks?.gold_per_min ?? null, 0],
          ['Damage / min', g.damage_to_champs / min, a.benchmarks?.damage_per_min ?? null, 0],
          ['Vision / min', g.vision / min, a.benchmarks?.vision_per_min ?? null, 2],
        ]
      : [],
  );
  const shares = $derived<[string, number | null][]>(
    a
      ? [
          ['Kill participation', a.kill_participation],
          ['Damage share', a.damage_share],
          ['Gold share', a.gold_share],
          ['Damage taken share', a.taken_share],
        ]
      : [],
  );
  const built = $derived(new Set(a?.players.find((p) => p.me)?.items ?? []));
  const epic = $derived(a?.marks.filter((m) => m.kind !== 'Tower' && m.kind !== 'Inhibitor') ?? []);
  const MARK = { there: '✓', dead: '✝', away: '✗' } as const;
  const WHERE = { there: 'You were there', dead: 'You were dead', away: 'You were elsewhere' } as const;
  const date = $derived(g ? new Date(g.created).toLocaleString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' }) : '');
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div class="review">
  <div class="bar">
    <button type="button" class="back" onclick={onclose}>‹ Back</button>
    {#if g}
      <Icon kind="champ" id={g.champ} size={52} />
      <div class="title">
        <div class="big">
          <span class={g.remake ? 'muted' : g.win ? 'good' : 'bad'}>{g.remake ? 'Remake' : g.win ? 'Victory' : 'Defeat'}</span>
          {name(g.champ)}{g.lane ? ` · ${LANE_NAMES[g.lane]}` : ''}
        </div>
        <div class="muted">{queueName(g.queue_id, g.mode)} · {clockText(g.duration_s)} · {date}</div>
      </div>
      {#if a?.opponent != null}
        <div class="vs muted">vs <Icon kind="champ" id={a.opponent} size={30} /></div>
      {/if}
      <div class="kda num">{g.kills}/{g.deaths}/{g.assists}</div>
    {/if}
  </div>

  {#if error}
    <section class="panel"><p class="bad">Could not load this game: {error}</p></section>
  {:else if !a || !g}
    <section class="panel"><p class="empty">Reading the game from the League client…</p></section>
  {:else}
    <div class="cols">
      <div class="col">
        <section class="panel">
          <h2>What stood out</h2>
          {#if a.takeaways.length}
            <ul class="takeaways">
              {#each a.takeaways as t, i (i)}
                <li class={t.good ? 'up' : 'down'}><span class="dot" aria-hidden="true">{t.good ? '▲' : '▼'}</span>{t.text}</li>
              {/each}
            </ul>
          {:else}
            <p class="empty">Nothing unusual: close to the average for {name(g.champ)} across the board.</p>
          {/if}
          {#if a.live}<p class="muted small">The coach watched this game live, so deaths show the gold you held and what Buy next said.</p>{/if}
        </section>

        <section class="panel">
          <h2>Your numbers</h2>
          <table class="stats">
            <tbody>
              {#each rows as [label, mine, avg, d] (label)}
                <tr>
                  <th>{label}</th>
                  <td class="num" class:good={avg != null && mine >= avg} class:bad={avg != null && mine < avg * 0.9}>{mine.toFixed(d)}</td>
                  <td class="num muted">{avg != null ? `avg ${avg.toFixed(d)}` : ''}</td>
                </tr>
              {/each}
              {#each shares as [label, v] (label)}
                {#if v != null}
                  <tr><th>{label}</th><td class="num">{(v * 100).toFixed(0)}%</td><td class="muted small">of your team</td></tr>
                {/if}
              {/each}
              {#if g.gold_diff_10 != null || g.gold_diff_15 != null}
                <tr>
                  <th>Gold vs {a.opponent != null ? name(a.opponent) : 'opponent'}</th>
                  <td class="num" class:good={(g.gold_diff_15 ?? 0) > 0} class:bad={(g.gold_diff_15 ?? 0) < 0}>
                    {g.gold_diff_15 != null ? (g.gold_diff_15 > 0 ? '+' : '') + g.gold_diff_15 : ''}
                  </td>
                  <td class="num muted">at 15:00{g.gold_diff_10 != null ? ` (${g.gold_diff_10 > 0 ? '+' : ''}${g.gold_diff_10} at 10)` : ''}</td>
                </tr>
              {/if}
            </tbody>
          </table>
          <div class="builds">
            <span class="muted">Built</span>
            <div class="items">{#each a.players.find((p) => p.me)?.items ?? [] as id, i (`${i}-${id}`)}<Icon kind="item" {id} size={30} />{/each}</div>
            {#if a.recommended.length}
              <span class="muted" title={a.live ? 'What the coach recommended during the game' : 'What the coach would build into this enemy team'}>Coach</span>
              <div class="items">
                {#each a.recommended as id, i (`${i}-${id}`)}
                  <span class:missed={!built.has(id)}><Icon kind="item" {id} size={30} /></span>
                {/each}
              </div>
            {/if}
          </div>
          <DeathReview review={a.review} />
          {#if a.review.deaths.some((d) => d.pos)}
            <div class="dmap">
              <DeathMap
                size={200}
                spots={a.review.deaths.flatMap((d) =>
                  d.pos ? [{ x: d.pos[0], y: d.pos[1], tone: d.traded ? 'win' : 'loss', title: `${clockText(d.time)}${d.place ? ` · ${d.place}` : ''}` }] : [],
                )}
              />
              <p class="muted small">Your deaths, red side turned around so your base is bottom-left. Green: trades.</p>
            </div>
          {/if}
        </section>
      </div>

      <div class="col">
        <section class="panel">
          <h2>Gold over the game</h2>
          {#if a.gold.length > 1}
            <GoldChart points={a.gold} kills={a.kills} marks={a.marks} duration={g.duration_s} opponent={a.opponent != null ? name(a.opponent) : null} />
          {:else}
            <p class="empty">The client has no timeline for this game.</p>
          {/if}
          {#if epic.length}
            <div class="objectives">
              {#each epic as m, i (i)}
                <span class="obj" class:ours={m.ours} title={m.presence ? WHERE[m.presence] : ''}>
                  <span class="num">{clockText(m.time)}</span>
                  {m.kind}{m.detail ? ` ${m.detail}` : ''}
                  {#if m.presence}<span class="where {m.presence}">{MARK[m.presence]}</span>{/if}
                </span>
              {/each}
            </div>
          {/if}
          {#if g.ours && g.theirs}
            <p class="muted small">
              Towers {g.ours.towers}–{g.theirs.towers} · Dragons {g.ours.dragons}–{g.theirs.dragons} · Barons {g.ours.barons}–{g.theirs.barons}
            </p>
          {/if}
        </section>
        <Fights fights={a.fights} />
        <section class="panel">
          <Scoreboard players={a.players} win={g.win} />
        </section>
      </div>
    </div>
  {/if}
</div>

<style>
  .review {
    display: grid;
    gap: var(--gap);
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  .back {
    padding: 6px 12px;
    border: 1px solid var(--line);
    border-radius: var(--r);
    background: var(--panel);
    cursor: pointer;
    font-weight: 600;
  }
  .back:hover {
    border-color: var(--gold-dim);
    color: var(--gold);
  }
  .title .big {
    font: 600 22px/1.1 var(--cond);
  }
  .vs {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-left: 8px;
  }
  .kda {
    margin-left: auto;
    font-size: 28px;
    font-weight: 600;
  }
  .cols {
    display: grid;
    grid-template-columns: minmax(320px, 2fr) minmax(0, 3fr);
    gap: var(--gap);
    align-items: start;
  }
  .col {
    display: grid;
    gap: var(--gap);
    min-width: 0;
  }
  .takeaways {
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    gap: 6px;
  }
  .takeaways li {
    display: flex;
    gap: 8px;
    align-items: baseline;
  }
  .dot {
    font-size: 11px;
  }
  .up .dot {
    color: var(--good);
  }
  .down .dot {
    color: var(--threat);
  }
  .small {
    font-size: 13px;
  }
  .stats {
    width: 100%;
    border-collapse: collapse;
    margin-bottom: 12px;
  }
  .stats th {
    text-align: left;
    font-weight: 400;
    color: var(--muted);
  }
  .stats td {
    text-align: right;
    font-size: 17px;
    padding: 1px 0 1px 12px;
  }
  .stats td.small {
    font-size: 13px;
  }
  .builds {
    display: grid;
    grid-template-columns: auto 1fr;
    align-items: center;
    gap: 6px 12px;
  }
  .items {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
  }
  .missed {
    display: inline-flex;
    border-radius: 4px;
    box-shadow: 0 0 0 2px var(--warn);
  }
  .dmap {
    display: flex;
    gap: 12px;
    align-items: flex-end;
    margin-top: 10px;
  }
  .objectives {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 10px;
  }
  .obj {
    padding: 1px 8px;
    border-radius: 999px;
    background: var(--raised);
    font-size: 13px;
    color: var(--threat);
  }
  .obj.ours {
    color: var(--good);
  }
  .where {
    margin-left: 3px;
    font-weight: 700;
  }
  .where.there {
    color: var(--good);
  }
  .where.dead,
  .where.away {
    color: var(--warn);
  }
  .obj .num {
    color: var(--muted);
    margin-right: 2px;
  }
  @media (max-width: 860px) {
    .cols {
      grid-template-columns: 1fr;
    }
  }
</style>
