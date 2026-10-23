<script lang="ts" module>
  // Calls already spoken this game, by key. Module-level: the banner unmounts
  // whenever there are no calls, and a remount must not speak them again.
  const spoken = new Set<string>();
  /** Fewest milliseconds between two spoken calls. */
  const GAP_MS = 6000;
  let lastSpoke = 0;
  /** Game clock last seen; going back means a new game. */
  let lastTime = 0;
</script>

<script lang="ts">
  // Banner of live calls (back for an objective, take one with numbers, play safe on a streak).
  // With audio on, each new call is spoken once per game.
  import type { Call } from '../../lib/bindings/Call';
  import { store } from '../../lib/store.svelte';

  let { calls }: { calls: Call[] } = $props();

  $effect(() => {
    const time = store.state?.game?.time ?? 0;
    if (time < lastTime) spoken.clear();
    lastTime = time;
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
    font: 600 18px/1.25 var(--cond);
  }
</style>
