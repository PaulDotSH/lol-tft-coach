<script lang="ts">
  // Shutdowns: players on kill streaks or far ahead in item gold, and those worth little.
  import type { Bounty } from '../../lib/bindings/Bounty';
  import Icon from '../Icon.svelte';

  let { bounties }: { bounties: Bounty[] } = $props();
</script>

<section class="panel">
  <h2>Shutdowns</h2>
  <ul>
    {#each bounties as b (b.champ)}
      <li class:ally={b.ally} class:low={b.cold >= 3 && b.streak < 3}>
        <Icon kind="champ" id={b.champ} size={26} />
        <span class="side small">{b.me ? 'you' : b.ally ? 'ally' : 'enemy'}</span>
        <span>{b.text}</span>
      </li>
    {/each}
  </ul>
</section>

<style>
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    gap: 5px;
  }
  li {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--threat);
  }
  li.ally {
    color: var(--good);
  }
  li.low {
    color: var(--muted);
  }
  .side {
    width: 40px;
    color: var(--faint);
  }
  .small {
    font-size: 12px;
  }
</style>
