<script lang="ts">
  // The one thing to work on next game; adds it as a goal when it maps to one.
  import type { Focus } from '../../lib/bindings/Focus';
  import { GOALS, MAX_GOALS, targetText } from '../../lib/goals';
  import { saveSettings, store } from '../../lib/store.svelte';

  let { focus }: { focus: Focus } = $props();
  const settings = $derived(store.state?.settings);
  const goal = $derived(focus.goal);
  const same = $derived(goal && settings?.goals.find((g) => g.kind === goal.kind));
  const full = $derived((settings?.goals.length ?? 0) >= MAX_GOALS);

  function add() {
    if (!settings || !goal) return;
    const rest = settings.goals.filter((g) => g.kind !== goal.kind);
    // Full: the new focus replaces the older goal.
    const goals = [...(rest.length >= MAX_GOALS ? rest.slice(1) : rest), goal];
    saveSettings({ ...settings, goals });
  }
</script>

<section class="panel focus">
  <h2>Focus for next game</h2>
  <p class="text">{focus.text}</p>
  <p class="muted small">{focus.why}</p>
  {#if goal && settings}
    {#if same && same.target === goal.target}
      <p class="muted small">Already one of your goals: tracked live in your next game.</p>
    {:else}
      <button type="button" class="add" onclick={add}>
        {same ? 'Update your goal' : full ? `Replace “${GOALS[settings.goals[0].kind].label}” with it` : 'Add as a goal'}
        <span class="muted">({GOALS[goal.kind].label} {targetText(goal.kind, goal.target)})</span>
      </button>
    {/if}
  {/if}
</section>

<style>
  .focus {
    border-left: 4px solid var(--gold);
  }
  .text {
    margin: 0;
    font: 600 20px/1.2 var(--cond);
    color: var(--gold);
  }
  .small {
    margin: 4px 0 0;
    font-size: 13px;
  }
  .add {
    margin-top: 10px;
    padding: 6px 12px;
    border: 1px solid var(--gold-dim);
    border-radius: var(--r);
    background: var(--raised);
    cursor: pointer;
    font-weight: 600;
  }
  .add:hover {
    border-color: var(--gold);
  }
</style>
