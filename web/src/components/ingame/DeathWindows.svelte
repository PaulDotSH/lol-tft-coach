<script lang="ts">
  // Who is dead and when they are back (scoreboard facts). One line per side, always present.
  import type { DeathWindow } from '../../lib/bindings/DeathWindow';
  import type { Respawn } from '../../lib/bindings/Respawn';
  import { clock } from '../../lib/store.svelte';
  import Icon from '../Icon.svelte';

  let { deaths, at }: { deaths: DeathWindow; at: number } = $props();
  const left = (r: Respawn) => Math.max(0, Math.ceil(r.in_s - (clock.time - at)));
  const sides = $derived([
    { name: 'enemies', one: 'enemy', list: deaths.enemies, cls: 'foe' },
    { name: 'allies', one: 'ally', list: deaths.allies, cls: 'ally' },
  ]);
</script>

<section class="panel deaths" aria-label="Death windows">
  {#if deaths.enemies.length === 0 && deaths.allies.length === 0}
    <p class="all">Everyone alive</p>
  {:else}
    {#each sides as s (s.name)}
      {#if s.list.length}
        <p class={s.cls}>
          <strong class="num">{s.list.length} {s.list.length === 1 ? s.one : s.name} dead</strong>
          <span class="muted">· next back in <span class="num">{left(s.list[0])} s</span></span>
          <span class="who">
            {#each s.list as r (r.champ)}
              <span class="r"><Icon kind="champ" id={r.champ} size={22} /><span class="num">{left(r)}</span></span>
            {/each}
          </span>
        </p>
      {/if}
    {/each}
  {/if}
</section>

<style>
  .deaths {
    display: grid;
    gap: 4px;
    min-height: 46px;
    align-content: center;
    padding: 8px 14px;
  }
  p {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 2px 8px;
    margin: 0;
    font-size: 16px;
  }
  .all {
    color: var(--faint);
  }
  .foe strong {
    color: var(--good);
  }
  .ally strong {
    color: var(--threat);
  }
  .who {
    display: inline-flex;
    gap: 8px;
  }
  .r {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font-size: 14px;
    color: var(--muted);
  }
</style>
