<script lang="ts">
  // Mock mode only: pick which fixture the fake backend pushes.
  import { mockFixtures, mockPhase } from '../mock';

  let value = $state(mockPhase.get());
</script>

<label>
  Mock
  <select bind:value onchange={() => mockPhase.set(value)}>
    {#each mockFixtures as f (f)}<option value={f}>{f}</option>{/each}
  </select>
</label>

<style>
  label {
    position: fixed;
    right: 10px;
    bottom: 10px;
    z-index: 10;
    padding: 4px 4px 4px 10px;
    border: 1px dashed var(--faint);
    border-radius: var(--r);
    background: var(--bg);
    font-size: 12px;
    color: var(--faint);
    opacity: 0.6;
  }
  label:hover,
  label:focus-within {
    opacity: 1;
  }
  select {
    background: var(--panel);
    border: 0;
    font-size: 12px;
  }
</style>
