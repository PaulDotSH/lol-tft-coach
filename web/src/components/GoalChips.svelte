<script lang="ts">
  // My goals for this game: live in the self-coach row, final in the recap. Silent: colour only.
  import type { GoalProgress } from '../lib/bindings/GoalProgress';
  import { GOALS, progressText, targetText } from '../lib/goals';

  let { goals, live = false }: { goals: GoalProgress[]; live?: boolean } = $props();
</script>

{#if goals.length}
  <div class="chips" aria-label="Goals">
    {#each goals as g (g.kind)}
      <div class="chip" class:met={g.met === true} class:missed={g.met === false}>
        <span class="label">{GOALS[g.kind].label}</span>
        <span class="num value">{progressText(g, live)}</span>
        <span class="target">{targetText(g.kind, g.target)}</span>
      </div>
    {/each}
  </div>
{/if}

<style>
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .chip {
    display: grid;
    grid-template-columns: auto auto;
    align-items: baseline;
    gap: 0 10px;
    padding: 5px 10px;
    border-radius: var(--r);
    border-left: 3px solid var(--faint);
    background: var(--raised);
  }
  .met {
    border-left-color: var(--good);
  }
  .missed {
    border-left-color: var(--threat);
  }
  .label {
    grid-column: 1 / -1;
    font: 600 12px var(--cond);
    letter-spacing: 0.03em;
    color: var(--muted);
  }
  .value {
    font-size: 19px;
    font-weight: 600;
  }
  .met .value {
    color: var(--good);
  }
  .missed .value {
    color: var(--threat);
  }
  .target {
    font-size: 12px;
    color: var(--faint);
  }
</style>
