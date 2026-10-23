<script lang="ts">
  // Banner of live calls (back for an objective, take one with numbers, play safe on a streak).
  // With audio on, each new call is spoken once.
  import type { Call } from '../../lib/bindings/Call';
  import { store } from '../../lib/store.svelte';

  let { calls }: { calls: Call[] } = $props();

  // Calls already spoken this session, by key.
  const spoken = new Set<string>();
  /** Fewest seconds between two spoken calls. */
  const GAP_MS = 6000;
  let lastSpoke = 0;

  $effect(() => {
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
