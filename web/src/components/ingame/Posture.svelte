<script lang="ts">
  // How the next minutes look, from the win chance: a read and the choices on the scoreboard, never orders.
  import type { Posture } from '../../lib/bindings/Posture';

  let { p }: { p: Posture } = $props();
  const LABEL = { press: 'Press', hold: 'Hold', even: 'Even', stall: 'Stall', swing: 'Swing' } as const;
</script>

<section class="panel posture {p.stance}" aria-label="Posture">
  <div class="top">
    <span class="badge">{LABEL[p.stance]}</span>
    <span class="text">{p.headline}</span>
  </div>
  {#if p.why.length}<p class="muted small">{p.why.join(' · ')}</p>{/if}
  {#if p.options.length}
    <ul>
      {#each p.options as o (o)}<li>{o}</li>{/each}
    </ul>
  {/if}
</section>

<style>
  .posture {
    display: grid;
    gap: 6px;
    align-content: start;
    border-left: 4px solid var(--muted);
  }
  .press,
  .hold {
    border-left-color: var(--good);
  }
  .stall {
    border-left-color: var(--warn);
  }
  .swing {
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
  .press .badge,
  .hold .badge {
    color: var(--good);
  }
  .stall .badge {
    color: var(--warn);
  }
  .swing .badge {
    color: var(--threat);
  }
  .text {
    font-weight: 600;
  }
  .small {
    margin: 0;
    font-size: 13px;
  }
  ul {
    margin: 0;
    padding: 0 0 0 18px;
    display: grid;
    gap: 3px;
  }
</style>
