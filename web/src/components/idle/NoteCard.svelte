<script lang="ts">
  // The coach's note: connected takeaways and a focus, each line's facts on hover.
  import type { GameNote } from '../../lib/bindings/GameNote';
  import type { NoteLine } from '../../lib/bindings/NoteLine';

  let { note }: { note: GameNote } = $props();
  const cites = (l: NoteLine) => note.facts.filter((f) => l.facts.includes(f.id));
</script>

<section class="panel note">
  <h2>Coach's note</h2>
  <ul>
    {#each note.takeaways as l, i (i)}
      <li class={l.good ? 'up' : 'down'}>
        <span class="dot" aria-hidden="true">{l.good ? '▲' : '▼'}</span>
        <span>{l.text}</span>
        {@render facts(l)}
      </li>
    {/each}
    {#if note.focus}
      <li class="focus">
        <span class="dot" aria-hidden="true">◆</span>
        <span>{note.focus.text}</span>
        {@render facts(note.focus)}
      </li>
    {/if}
  </ul>
  <p class="muted small">Built from the facts of this game only; hover a line to see them.</p>
</section>

{#snippet facts(l: NoteLine)}
  <ol class="cites" aria-label="Facts behind this line">
    {#each cites(l) as f (f.id)}
      <li><span class="num">{f.id}</span>{f.text}</li>
    {/each}
  </ol>
{/snippet}

<style>
  .note {
    border-left: 4px solid var(--gold-dim);
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    gap: 8px;
  }
  ul > li {
    position: relative;
    display: flex;
    gap: 8px;
    align-items: baseline;
    border-radius: var(--r);
  }
  ul > li:hover {
    background: var(--raised);
  }
  .dot {
    font-size: 11px;
  }
  .up .dot {
    color: var(--good);
  }
  .down .dot {
    color: var(--threat);
  }
  .focus {
    color: var(--gold);
    font-weight: 600;
  }
  .focus .dot {
    color: var(--gold);
  }
  .cites {
    display: none;
    position: absolute;
    z-index: 5;
    top: 100%;
    left: 18px;
    right: 0;
    margin: 2px 0 0;
    padding: 8px 10px;
    list-style: none;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--r);
    box-shadow: 0 6px 18px rgb(0 0 0 / 0.4);
    color: var(--muted);
    font-size: 13px;
    font-weight: 400;
  }
  ul > li:hover .cites {
    display: grid;
    gap: 3px;
  }
  .cites .num {
    color: var(--faint);
    margin-right: 6px;
  }
  .small {
    margin: 8px 0 0;
    font-size: 12px;
  }
</style>
