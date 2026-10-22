<script lang="ts">
  // Personal trends per mode and my 1–2 goals (picked here, saved in settings).
  import type { Goal } from '../../lib/bindings/Goal';
  import type { GoalKind } from '../../lib/bindings/GoalKind';
  import type { Mode } from '../../lib/bindings/Mode';
  import type { Settings } from '../../lib/bindings/Settings';
  import type { Trend } from '../../lib/bindings/Trend';
  import type { Trends } from '../../lib/bindings/Trends';
  import { fmtGoal, GOAL_KINDS, GOALS, MAX_GOALS, targetText } from '../../lib/goals';
  import { saveSettings, store } from '../../lib/store.svelte';
  import { MODE_NAMES } from '../../lib/ui';
  import Icon from '../Icon.svelte';

  let { trends, settings }: { trends: Trends | null; settings: Settings } = $props();

  const goals = $derived(settings.goals);
  const unused = $derived(GOAL_KINDS.filter((k) => !goals.some((g) => g.kind === k)));
  const save = (goals: Goal[]) => saveSettings({ ...settings, goals });
  const add = (kind: GoalKind) => save([...goals, { kind, target: GOALS[kind].target }]);
  const remove = (i: number) => save(goals.filter((_, j) => j !== i));
  function setTarget(i: number, raw: string) {
    const v = Number(raw);
    if (Number.isFinite(v) && v >= 0) save(goals.map((g, j) => (j === i ? { ...g, target: v } : g)));
  }
  const status = (kind: GoalKind) => trends?.goals.find((s) => s.goal.kind === kind);

  const modes = $derived.by(() => {
    const m = new Map<Mode, Trend[]>();
    for (const t of trends?.trends ?? []) m.set(t.mode, [...(m.get(t.mode) ?? []), t]);
    return [...m];
  });

  const W = 96;
  const H = 24;
  function spark(series: number[]) {
    const lo = Math.min(...series);
    const span = Math.max(...series) - lo || 1;
    const step = series.length > 1 ? W / (series.length - 1) : 0;
    return series.map((v, i) => `${(i * step).toFixed(1)},${(H - 2 - ((v - lo) / span) * (H - 4)).toFixed(1)}`).join(' ');
  }
  const augName = (id: number) => store.ui?.augments[id] ?? `Augment ${id}`;
</script>

<section class="panel">
  <h2>Trends and goals</h2>

  <div class="goals">
    {#each goals as g, i (g.kind)}
      {@const s = status(g.kind)}
      <div class="goal">
        <span class="gname">{GOALS[g.kind].label}</span>
        <label class="target">
          <span class="muted">{GOALS[g.kind].lower ? (g.kind === 'unspent' ? 'under' : 'at most') : 'at least'}</span>
          <input
            type="number"
            min="0"
            step={GOALS[g.kind].step}
            value={g.target}
            onchange={(e) => setTarget(i, e.currentTarget.value)}
            aria-label="{GOALS[g.kind].label} target"
          />
        </label>
        <span class="gstat muted">
          {#if s && s.of}met in <b class="num">{s.met}</b> of last {s.of}{:else}no games yet{/if}
          {#if GOALS[g.kind].riftOnly}<span class="faint">· Rift</span>{/if}
        </span>
        <button type="button" class="x" onclick={() => remove(i)} aria-label="Remove goal">✕</button>
      </div>
    {/each}
    {#if goals.length < MAX_GOALS}
      <select
        class="add"
        value=""
        onchange={(e) => {
          const k = e.currentTarget.value as GoalKind;
          e.currentTarget.value = '';
          if (k) add(k);
        }}
        aria-label="Add a goal"
      >
        <option value="" disabled>{goals.length ? 'Add a second goal…' : 'Pick a goal to track…'}</option>
        {#each unused as k (k)}<option value={k}>{GOALS[k].label} ({targetText(k, GOALS[k].target)})</option>{/each}
      </select>
    {/if}
  </div>

  {#if !trends || (!modes.length && !trends.follow.length && !trends.augments.length)}
    <p class="empty">Trends appear once a mode has 3+ games.</p>
  {:else}
    {#each modes as [mode, list] (mode)}
      <h3>{MODE_NAMES[mode]}</h3>
      <div class="trends">
        {#each list as t (t.kind)}
          <span class="tname">{GOALS[t.kind].label}</span>
          <svg width={W} height={H} viewBox="0 0 {W} {H}" aria-hidden="true">
            <polyline points={spark(t.series)} class:up={t.better === true} class:down={t.better === false} />
          </svg>
          <span class="num now" class:good={t.better === true} class:bad={t.better === false}>{fmtGoal(t.kind, t.recent)}</span>
          <span class="num muted before">{t.before != null ? `was ${fmtGoal(t.kind, t.before)}` : ''}</span>
        {/each}
      </div>
    {/each}

    {#each trends.follow as f (f.mode)}
      <p class="follow">
        <span class="muted">Followed Buy next ({MODE_NAMES[f.mode]}, {f.games} games):</span>
        <span class="num">{f.wins != null ? `${f.wins.toFixed(0)}%` : '—'}</span> in wins,
        <span class="num">{f.losses != null ? `${f.losses.toFixed(0)}%` : '—'}</span> in losses
      </p>
    {/each}

    {#if trends.augments.length}
      <h3>Mayhem augments you took <span class="faint">games · damage/min</span></h3>
      <div class="augs">
        {#each trends.augments as a (a.id)}
          <div class="aug">
            <Icon kind="augment" id={a.id} size={24} />
            <span class="aname">{augName(a.id)}</span>
            <span class="num">{a.games}</span>
            <span class="num muted">{a.damage_per_min.toFixed(0)}</span>
          </div>
        {/each}
      </div>
    {/if}
  {/if}
</section>

<style>
  h3 {
    margin: 14px 0 6px;
    font: 600 14px var(--cond);
    color: var(--muted);
  }
  .faint {
    color: var(--faint);
    font-weight: 400;
  }
  .goals {
    display: grid;
    gap: 6px;
  }
  .goal {
    display: grid;
    grid-template-columns: 1fr auto auto;
    align-items: center;
    gap: 2px 10px;
    padding: 6px 10px;
    border-radius: var(--r);
    border-left: 3px solid var(--gold);
    background: var(--raised);
  }
  .gname {
    font: 600 16px var(--cond);
  }
  .target {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
  }
  .target input {
    width: 70px;
    padding: 2px 6px;
    border: 1px solid var(--line);
    border-radius: 4px;
    background: var(--bg);
  }
  .gstat {
    grid-column: 1 / 3;
    font-size: 13px;
  }
  .x {
    grid-row: 1;
    grid-column: 3;
    border: 0;
    background: none;
    color: var(--faint);
    cursor: pointer;
  }
  .x:hover {
    color: var(--text);
  }
  .add {
    padding: 4px 6px;
    border: 1px dashed var(--line);
    border-radius: var(--r);
    background: var(--panel);
    color: var(--muted);
  }
  .trends {
    display: grid;
    grid-template-columns: 1fr auto auto auto;
    align-items: center;
    gap: 4px 12px;
  }
  .tname {
    font-size: 14px;
  }
  polyline {
    fill: none;
    stroke: var(--faint);
    stroke-width: 1.5;
    stroke-linejoin: round;
  }
  polyline.up {
    stroke: var(--good);
  }
  polyline.down {
    stroke: var(--threat);
  }
  .now {
    font-size: 17px;
    font-weight: 600;
    text-align: right;
  }
  .before {
    font-size: 13px;
    min-width: 70px;
  }
  .follow {
    margin: 12px 0 0;
    font-size: 14px;
  }
  .augs {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 4px 14px;
  }
  .aug {
    display: grid;
    grid-template-columns: auto 1fr auto auto;
    align-items: center;
    gap: 8px;
    font-size: 14px;
  }
  .aname {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
