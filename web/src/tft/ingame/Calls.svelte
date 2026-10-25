<script lang="ts" module>
  // Calls already spoken this game, by key. Module-level: the banner unmounts
  // whenever there are no calls, and a remount must not speak them again.
  const spoken = new Set<string>();
  /** Fewest milliseconds between two spoken calls. */
  const GAP_MS = 6000;
  let lastSpoke = 0;
  /** Stage last seen as stage * 10 + round; going back means a new game. */
  let lastStage = 0;
</script>

<script lang="ts">
  // Banner of TFT key moments (level / roll down now, a contested unit, take this augment).
  // With audio on (the shared LoL setting), each new call is spoken once per game.
  import type { TftCall } from '../../lib/tft-bindings/TftCall';
  import { store } from '../../lib/store.svelte';

  let { calls, stage }: { calls: TftCall[]; stage: [number, number] | null } = $props();

  $effect(() => {
    const at = stage ? stage[0] * 10 + stage[1] : lastStage;
    if (at < lastStage) spoken.clear();
    lastStage = at;
    if (!store.state?.settings.audio || typeof speechSynthesis === 'undefined') return;
    const fresh = calls.filter((c) => !spoken.has(c.key));
    if (!fresh.length || Date.now() - lastSpoke < GAP_MS) return;
    const c = fresh[0];
    spoken.add(c.key);
    lastSpoke = Date.now();
    const u = new SpeechSynthesisUtterance(c.say);
    u.rate = 1.1;
    speechSynthesis.speak(u);
  });
</script>

{#if calls.length}
  <section class="calls" aria-live="polite">
    {#each calls as c (c.key)}
      <p>{c.text}</p>
    {/each}
  </section>
{/if}

<style>
  .calls {
    display: grid;
    gap: 4px;
    padding: 8px 14px;
    border-radius: var(--r);
    border-left: 4px solid var(--gold);
    background: color-mix(in srgb, var(--gold) 14%, var(--panel));
  }
  p {
    margin: 0;
    font: 600 22px/1.25 var(--cond);
  }
</style>
