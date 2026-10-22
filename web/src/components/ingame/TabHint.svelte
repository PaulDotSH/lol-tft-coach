<script lang="ts">
  // Mayhem: silent prompt to hold Tab so the enemy augments can be read off the scoreboard.
  // Disappears on its own once they were read.
  import type { TabHint } from '../../lib/bindings/TabHint';

  let { hint }: { hint: TabHint } = $props();
</script>

<section class="hint" aria-live="polite">
  <kbd>Tab</kbd>
  <div>
    <div class="text">{hint.text}</div>
    <div class="reason">{hint.reason}</div>
  </div>
</section>

<style>
  .hint {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
    border-radius: var(--r);
    background: var(--panel);
    box-shadow: inset 3px 0 0 var(--gold);
  }
  kbd {
    flex: none;
    min-width: 46px;
    padding: 4px 8px 5px;
    border-radius: 5px;
    background: var(--raised);
    box-shadow:
      inset 0 -2px 0 rgb(0 0 0 / 0.35),
      0 0 0 1px var(--line);
    font: 700 16px var(--cond);
    text-align: center;
    color: var(--gold);
  }
  .text {
    font: 600 18px/1.15 var(--cond);
  }
  .reason {
    font-size: 13px;
    color: var(--muted);
  }
</style>
