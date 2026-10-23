<script lang="ts">
  // Quiet tips (not spoken): CS pace, vision and Control Wards, gold held too long.
  import type { LiveTip } from '../../lib/bindings/LiveTip';

  let { tips }: { tips: LiveTip[] } = $props();
  const LABEL: Record<string, string> = { lane: 'Lane', vision: 'Vision', back: 'Back' };
</script>

<section class="panel">
  <h2>Tips</h2>
  <ul>
    {#each tips as t (t.key)}
      <li class={t.kind}><span class="kind">{LABEL[t.kind] ?? t.kind}</span><span>{t.text}</span></li>
    {/each}
  </ul>
</section>

<style>
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    gap: 6px;
  }
  li {
    display: grid;
    grid-template-columns: 52px 1fr;
    gap: 8px;
    align-items: baseline;
    font-size: 15px;
  }
  .kind {
    font: 700 12px var(--cond);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
  }
  .back .kind {
    color: var(--warn);
  }
  .vision .kind {
    color: var(--gold);
  }
</style>
