<script lang="ts">
  // The key line, readable from across the desk: what to do with gold right now
  // (level / roll / save / hold) with its reason, next to the stage and the team size
  // (units on board / cap; highlighted while a slot is free).
  import type { EconAdvice } from '../../lib/tft-bindings/EconAdvice';
  import { stageText } from '../../lib/tft';

  let {
    econ,
    stage,
    team = null,
  }: { econ: EconAdvice | null; stage: [number, number] | null; team?: [number, number] | null } = $props();

  const LABELS = { level: 'Level', roll: 'Roll', save: 'Save', hold: 'Hold' } as const;
</script>

<section class="econ {econ?.action ?? 'none'}" aria-label="Gold">
  {#if stage}<span class="stage num" title="Stage">{stageText(stage)}</span>{/if}
  {#if team}<span class="team num" class:free={team[0] < team[1]} title="Units on board / team size">{team[0]}/{team[1]}</span>{/if}
  {#if econ}
    <span class="action">{LABELS[econ.action]}</span>
    <p class="text">{econ.text}</p>
  {:else}
    <p class="text empty">Reading gold, level and stage…</p>
  {/if}
</section>

<style>
  .econ {
    --accent: var(--faint);
    display: flex;
    align-items: center;
    gap: 18px;
    padding: 12px 18px;
    border-radius: 10px;
    border: 1px solid color-mix(in srgb, var(--accent) 60%, var(--line));
    background: linear-gradient(90deg, color-mix(in srgb, var(--accent) 18%, var(--panel)) 0%, var(--panel) 70%);
  }
  .level {
    --accent: var(--gold);
  }
  .roll {
    --accent: var(--threat);
  }
  .save {
    --accent: var(--good);
  }
  .hold {
    --accent: var(--muted);
  }
  .stage {
    font-size: 26px;
    font-weight: 600;
    color: var(--muted);
  }
  .team {
    font-size: 26px;
    font-weight: 600;
    color: var(--muted);
  }
  .team.free {
    color: var(--gold);
  }
  .action {
    flex: none;
    padding: 2px 14px;
    border-radius: var(--r);
    background: var(--accent);
    color: var(--bg);
    font: 700 34px/1.1 var(--cond);
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }
  .text {
    margin: 0;
    min-width: 0;
    font: 600 28px/1.2 var(--cond);
  }
  .text.empty {
    font: italic 18px var(--sans);
  }
</style>
