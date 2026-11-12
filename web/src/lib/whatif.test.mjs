import assert from 'node:assert/strict';
import test from 'node:test';
import { ehpLine, figure, gainText } from './whatif.ts';

const w = (o) => ({ item: 1, damage: 0, burst: 0, sustained: 0, burst_leads: true, per_enemy: [], ehp: 0, ehp_physical: 0, ehp_magic: 0, rough: false, note: null, ...o });

test('damage items show damage, defensive ones EHP, both when an item gives both', () => {
  assert.equal(figure(w({ damage: 0.091 })), '+9 % dmg');
  assert.equal(figure(w({ ehp: 0.18 })), '+18 % EHP');
  assert.equal(figure(w({ damage: 0.12, ehp: 0.07 })), '+12 % dmg · +7 % EHP');
});

test('no gain shows nothing, rough profiles are marked', () => {
  assert.equal(figure(w({})), '');
  assert.equal(figure(undefined), '');
  assert.equal(figure(w({ damage: 0.003 })), '');
  assert.equal(figure(w({ damage: 0.2, rough: true })), '~+20 % dmg');
});

test('EHP line names the mix and both damage types', () => {
  assert.equal(ehpLine(w({ ehp: 0.18, ehp_magic: 0.31, ehp_physical: 0 })), '+18 % EHP vs their mix (+31 % vs magic, ±0 % vs physical)');
  assert.equal(ehpLine(w({})), '');
  assert.equal(gainText(-0.04), '−4 %');
});
