<script lang="ts">
  import type { Settings } from '../../lib/bindings/Settings';
  import type { Sources } from '../../lib/bindings/Sources';
  import { coachless, saveSettings } from '../../lib/store.svelte';

  let { settings, sources }: { settings: Settings; sources: Sources } = $props();
  const signedIn = $derived(sources.coachless !== 'off');

  const TIERS = [
    'all', 'iron', 'bronze', 'silver', 'gold', 'platinum', 'platinum_plus', 'emerald', 'emerald_plus',
    'diamond', 'diamond_plus', 'master', 'master_plus', 'grandmaster', 'challenger',
  ];
  const REGIONS = ['all', 'na', 'euw', 'eune', 'kr', 'br', 'lan', 'las', 'oce', 'jp', 'tr', 'ru', 'me', 'sg', 'tw', 'vn'];
  const tierName = (t: string) => (t === 'all' ? 'All ranks' : t[0].toUpperCase() + t.slice(1).replace('_plus', '+'));
  const regionName = (r: string) => (r === 'all' ? 'All regions' : r.toUpperCase());

  const set = <K extends keyof Settings>(k: K, v: Settings[K]) => saveSettings({ ...settings, [k]: v });
</script>

<section class="panel">
  <h2>Settings</h2>
  <div class="grid">
    <label for="tier">Stats from</label>
    <select id="tier" value={settings.tier} onchange={(e) => set('tier', e.currentTarget.value)}>
      {#each TIERS as t (t)}<option value={t}>{tierName(t)}</option>{/each}
    </select>
    <label for="region">Region</label>
    <select id="region" value={settings.region} onchange={(e) => set('region', e.currentTarget.value)}>
      {#each REGIONS as r (r)}<option value={r}>{regionName(r)}</option>{/each}
    </select>
    <label for="coachless">
      Coachless builds
      <small>
        {#if !signedIn}Rift items, runes and summoners from coachless.gg; lolalytics until you sign in
        {:else if sources.coachless_premium}Signed in, Premium: current patch
        {:else}Signed in, free account: the patch before the newest{/if}
      </small>
    </label>
    <button id="coachless" onclick={() => coachless(!signedIn)}>{signedIn ? 'Sign out' : 'Sign in'}</button>
    <label for="audio">Speak in-game calls aloud</label>
    <input id="audio" type="checkbox" checked={settings.audio} onchange={(e) => set('audio', e.currentTarget.checked)} />
    <label for="overlay">
      In-game overlay over the game window
      <small>borderless or windowed; exclusive fullscreen hides it</small>
    </label>
    <input id="overlay" type="checkbox" checked={settings.overlay} onchange={(e) => set('overlay', e.currentTarget.checked)} />
    <label for="ocr">Read Mayhem augments from screen</label>
    <input id="ocr" type="checkbox" checked={settings.ocr_enabled} onchange={(e) => set('ocr_enabled', e.currentTarget.checked)} />
    <label for="gank">
      Gank warnings
      <small>reads the minimap on the Rift and warns of ganks on your lane, the enemy jungler, roams and enemies in your jungle</small>
    </label>
    <input id="gank" type="checkbox" checked={settings.gank_warn} onchange={(e) => set('gank_warn', e.currentTarget.checked)} />
    <label for="rec">Record games as test fixtures</label>
    <input id="rec" type="checkbox" checked={settings.record_fixtures} onchange={(e) => set('record_fixtures', e.currentTarget.checked)} />
    <label for="crops">
      Collect TFT unit crops
      <small>saves unit pictures of each fight and scouted board for training unit recognition; always on in debug mode</small>
    </label>
    <input id="crops" type="checkbox" checked={settings.tft_crops} onchange={(e) => set('tft_crops', e.currentTarget.checked)} />
  </div>
</section>

<style>
  .grid {
    display: grid;
    grid-template-columns: 1fr auto;
    align-items: center;
    gap: 8px 16px;
  }
  label {
    color: var(--muted);
  }
  small {
    display: block;
    color: var(--faint);
    font-size: 12px;
  }
  select {
    min-width: 140px;
    padding: 3px 6px;
    border: 1px solid var(--line);
    border-radius: 4px;
    background: var(--raised);
  }
  button {
    justify-self: end;
    padding: 3px 12px;
    border: 1px solid var(--line);
    border-radius: 4px;
    background: var(--raised);
    color: inherit;
    cursor: pointer;
  }
  input[type='checkbox'] {
    justify-self: end;
    width: 18px;
    height: 18px;
    accent-color: var(--gold);
  }
</style>
