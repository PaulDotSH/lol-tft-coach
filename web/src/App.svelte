<script lang="ts">
  import Header from './components/Header.svelte';
  import InGame from './components/ingame/InGame.svelte';
  import ChampSelect from './components/select/ChampSelect.svelte';
  import Idle from './components/idle/Idle.svelte';
  import { isTauri } from './lib/api';
  import { store } from './lib/store.svelte';

  const s = $derived(store.state);
  const DevSwitcher = isTauri ? null : import('./components/DevSwitcher.svelte');
</script>

{#if s}
  <Header state={s} />
  <main>
    {#if s.phase === 'ingame' && s.game}
      <InGame game={s.game} />
    {:else if s.phase === 'champselect' && s.champ_select}
      <ChampSelect cs={s.champ_select} />
    {:else}
      <Idle state={s} />
    {/if}
  </main>
{:else}
  <p class="boot muted">{store.error ? `Could not reach the backend: ${store.error}` : 'Starting…'}</p>
{/if}

{#if DevSwitcher}
  {#await DevSwitcher then m}
    <m.default />
  {/await}
{/if}

<style>
  main {
    flex: 1;
    display: flex;
    flex-direction: column;
    padding: var(--gap);
    min-height: 0;
  }
  .boot {
    margin: auto;
  }
</style>
