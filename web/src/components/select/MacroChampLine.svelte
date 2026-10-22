<script lang="ts">
  // One line in champ select: my recent record on this champion in this mode (macro coach).
  import type { Mode } from '../../lib/bindings/Mode';
  import { store } from '../../lib/store.svelte';
  import { MODE_NAMES } from '../../lib/ui';

  let { champ, mode }: { champ: number; mode: Mode } = $props();

  const report = $derived(store.state?.macro_report ?? null);
  const rec = $derived(report?.champs.find((c) => c.mode === mode && c.champ === champ));
  const pattern = $derived(report?.patterns.find((p) => p.kind === 'champion' && p.mode === mode && p.champ === champ));
</script>

{#if rec}
  <div class="line">
    <span class="muted">Your recent {MODE_NAMES[mode]} games on this champion:</span>
    <span class="num">{rec.wins}W {rec.games - rec.wins}L</span>
    {#if pattern}<span class="note">· {pattern.evidence.replace(/^[^:]*: /, '')}</span>{/if}
  </div>
{/if}

<style>
  .line {
    margin-top: 2px;
    font-size: 13px;
  }
  .num {
    font-size: 15px;
  }
  .note {
    color: var(--warn);
  }
</style>
