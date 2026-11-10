<script lang="ts">
  import type { SkillNext } from '../../lib/bindings/SkillNext';

  let { skill }: { skill: SkillNext } = $props();
  const order = $derived(skill.priority.split('').join(' > '));
</script>

<div class="skill" class:due={skill.unspent > 0} title={skill.source} aria-label="Next skill point">
  {#if skill.unspent > 0}
    <span>Level up <strong class="key">{skill.key}</strong></span>
    {#if skill.unspent > 1}<span class="num">({skill.unspent} points)</span>{/if}
  {:else}
    <span>Next point: <strong class="key">{skill.key}</strong> at <span class="num">{skill.at_level}</span></span>
  {/if}
  {#if order}<span class="order">· {order}</span>{/if}
</div>

<style>
  .skill {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0 8px;
    padding: 6px 12px;
    border-radius: var(--r);
    background: var(--panel);
    color: var(--muted);
    font-size: 16px;
  }
  .key {
    font: 700 19px var(--cond);
  }
  .due {
    color: var(--gold);
    border: 1px solid var(--gold-dim);
  }
  .due .key {
    font-size: 24px;
  }
  .order {
    color: var(--muted);
  }
</style>
