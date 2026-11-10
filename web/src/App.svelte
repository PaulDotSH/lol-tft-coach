<script lang="ts">
  import Header from './components/Header.svelte';
  import InGame from './components/ingame/InGame.svelte';
  import ChampSelect from './components/select/ChampSelect.svelte';
  import Idle from './components/idle/Idle.svelte';
  import GamePlan from './components/GamePlan.svelte';
  import { isTauri } from './lib/api';
  import { store } from './lib/store.svelte';
  import { tft } from './lib/tft-store.svelte';
  import Tft from './tft/Tft.svelte';

  const s = $derived(store.state);
  // In TFT the LoL coach goes quiet and the TFT screens take over (header stays); the TFT
  // client idling in its menus doesn't cover a LoL champ select or game.
  const lol = $derived(s && (s.phase === 'champselect' || s.phase === 'loading' || s.phase === 'ingame'));
  const t = $derived(
    tft.state && (tft.state.phase === 'ingame' || (tft.state.phase !== 'none' && !lol)) ? tft.state : null,
  );
  const DevSwitcher = isTauri ? null : import('./components/DevSwitcher.svelte');
</script>

{#if s}
  <Header state={s} />
  <main>
    {#if t}
      <Tft state={t} />
    {:else if s.phase === 'ingame' && s.game}
      {#if s.plan}<div class="plan"><GamePlan plan={s.plan} /></div>{/if}
      <InGame game={s.game} />
    {:else if s.phase === 'champselect' && s.champ_select}
      <ChampSelect cs={s.champ_select} />
    {:else}
      {#if s.phase === 'loading' && s.plan}<div class="plan"><GamePlan plan={s.plan} full /></div>{/if}
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
  .plan {
    margin-bottom: var(--gap);
  }
  .boot {
    margin: auto;
  }
</style>
