<script lang="ts">
  // Fight readiness: item gold, levels and who is dead, and whether to fight now.
  import type { Readiness } from '../../lib/bindings/Readiness';

  let { r }: { r: Readiness } = $props();
  const LABEL = { fight: 'Fight', even: 'Even', avoid: 'Avoid' } as const;
</script>

<section class="panel ready {r.verdict}">
  <div class="top">
    <span class="badge">{LABEL[r.verdict]}</span>
    <span class="text">{r.text}</span>
  </div>
  {#if r.reasons.length}<p class="muted small">{r.reasons.join(' · ')}</p>{/if}
</section>

<style>
  .ready {
    border-left: 4px solid var(--muted);
  }
  .ready.fight {
    border-left-color: var(--good);
  }
  .ready.avoid {
    border-left-color: var(--threat);
  }
  .top {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  .badge {
    font: 700 17px var(--cond);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .fight .badge {
    color: var(--good);
  }
  .avoid .badge {
    color: var(--threat);
  }
  .text {
    font-weight: 600;
  }
  .small {
    margin: 4px 0 0;
    font-size: 13px;
  }
</style>
