<script lang="ts">
  import type { Settings } from '../../lib/bindings/Settings';
  import { saveSettings } from '../../lib/store.svelte';

  let { settings }: { settings: Settings } = $props();

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
    <label for="ocr">Read Mayhem augments from screen</label>
    <input id="ocr" type="checkbox" checked={settings.ocr_enabled} onchange={(e) => set('ocr_enabled', e.currentTarget.checked)} />
    <label for="rec">Record games as test fixtures</label>
    <input id="rec" type="checkbox" checked={settings.record_fixtures} onchange={(e) => set('record_fixtures', e.currentTarget.checked)} />
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
  select {
    min-width: 140px;
    padding: 3px 6px;
    border: 1px solid var(--line);
    border-radius: 4px;
    background: var(--raised);
  }
  input[type='checkbox'] {
    justify-self: end;
    width: 18px;
    height: 18px;
    accent-color: var(--gold);
  }
</style>
