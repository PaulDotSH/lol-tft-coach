<script lang="ts">
  // Tips against one lane opponent (champ select and the in-game laning panel): my win rate,
  // their lane tags, the pair note, how they win / spike / when to trade, their spell
  // cooldowns (spells to play around marked) and my record against them.
  import type { MatchupCard } from '../lib/bindings/MatchupCard';
  import { store } from '../lib/store.svelte';
  import { champName, cooldownText, gold, signed, spellIcon } from '../lib/ui';
  import Icon from './Icon.svelte';

  let { card, me }: { card: MatchupCard; me: number | null } = $props();

  /** Win rates from fewer games are shown greyed. */
  const FEW = 100;
  const name = (c: number) => (store.ui ? champName(store.ui, c) : '');
  const games = (n: number) => (n >= 1000 ? `${(n / 1000).toFixed(1)}k` : `${n}`);
  const r = $derived(card.record);
  const few = $derived(card.games < FEW);
</script>

<article class="card">
  <div class="head">
    <Icon kind="champ" id={card.champ} size={40} />
    <span class="who">{name(card.champ)}</span>
    {#if card.vs_wr != null}
      <span class="num wr" class:few class:good={!few && card.vs_wr >= 51} class:bad={!few && card.vs_wr <= 49}>{card.vs_wr.toFixed(1)}%</span>
      <span class="muted small">{games(card.games)} games{few ? ', few' : ''}</span>
    {/if}
    {#each card.tags as t (t)}<span class="tag">{t}</span>{/each}
  </div>

  {#if card.pair}
    <p class="pair"><span class="label">{me != null ? name(me) : 'You'} vs {name(card.champ)}</span>{card.pair}</p>
  {/if}

  {#if card.note}
    <dl>
      <dt>How they win</dt>
      <dd>{card.note.wins}</dd>
      <dt>Spike</dt>
      <dd>{card.note.spike}</dd>
      <dt>Trade</dt>
      <dd>{card.note.trade}</dd>
    </dl>
  {/if}

  {#if card.spells.length}
    <ul class="spells">
      {#each card.spells as s (s.key)}
        <li class:watch={s.watch} title="{s.name}{s.watch ? ': play around it' : ''}">
          {#if store.ui}<img src={spellIcon(store.ui, s.icon)} alt="" width="26" height="26" />{/if}
          <span class="key">{s.key}</span>
          <span class="num cd">{s.now != null ? `${Math.round(s.now)}s now` : cooldownText(s.cooldown)}</span>
        </li>
      {/each}
    </ul>
  {/if}

  {#if r}
    <p class="record small">
      <span class="label">You</span>
      {#if r.same_games && me != null}
        as {name(me)} <span class="good">{r.same_wins}W</span> <span class="bad">{r.same_games - r.same_wins}L</span> ·
      {/if}
      all <span class="good">{r.wins}W</span> <span class="bad">{r.games - r.wins}L</span>
      {#if r.gold_15 != null}· <span class:good={r.gold_15 > 0} class:bad={r.gold_15 < 0}>{signed(r.gold_15, gold)}</span> at 15{/if}
      {#if r.early_kills}· <span class="warn">killed you {r.early_kills}× before 15:00</span>{:else if r.killed_me}· killed you {r.killed_me}×{/if}
      {#if r.last.length}
        · last
        {#each r.last as w, i (i)}<span class="res" class:good={w} class:bad={!w}>{w ? 'W' : 'L'}</span>{/each}
      {/if}
    </p>
  {/if}
</article>

<style>
  .card {
    display: grid;
    gap: 6px;
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .who {
    font: 600 18px var(--cond);
  }
  .wr {
    font-size: 18px;
    font-weight: 600;
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
  .small {
    font-size: 13px;
  }
  .label {
    margin-right: 6px;
    font: 700 12px var(--cond);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
  }
  .pair {
    margin: 0;
    padding: 6px 8px;
    border-left: 3px solid var(--gold);
    border-radius: 0 var(--r) var(--r) 0;
    background: var(--raised);
    font-size: 14px;
  }
  .pair .label {
    display: block;
    color: var(--gold);
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 3px 10px;
    margin: 0;
    font-size: 14px;
  }
  dt {
    font: 700 12px/1.6 var(--cond);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
  }
  dd {
    margin: 0;
  }
  .spells {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 12px;
    margin: 2px 0 0;
    padding: 0;
    list-style: none;
  }
  .spells li {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 2px 6px 2px 2px;
    border-radius: var(--r);
  }
  .spells img {
    border-radius: 4px;
  }
  .spells .watch {
    background: color-mix(in srgb, var(--warn) 22%, transparent);
    box-shadow: inset 0 0 0 1px var(--warn);
  }
  .key {
    font: 700 13px var(--cond);
  }
  .watch .key {
    color: var(--warn);
  }
  .cd {
    font-size: 14px;
  }
  .record {
    margin: 0;
  }
  .warn {
    color: var(--warn);
  }
  .res {
    margin-left: 3px;
    font: 700 13px var(--cond);
  }
</style>
