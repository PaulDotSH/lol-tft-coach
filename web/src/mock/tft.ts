// TFT samples for the plain-browser mock: a small hand-made Set 18 slice (names, costs, traits,
// Community Dragon icon paths) and one TftState per TFT screen.
import type { CompPick } from '../lib/tft-bindings/CompPick';
import type { TftAdvice } from '../lib/tft-bindings/TftAdvice';
import type { TftAugment } from '../lib/tft-bindings/TftAugment';
import type { TftItem } from '../lib/tft-bindings/TftItem';
import type { TftPastGame } from '../lib/tft-bindings/TftPastGame';
import type { TftSetData } from '../lib/tft-bindings/TftSetData';
import type { TftState } from '../lib/tft-bindings/TftState';
import type { TftTrait } from '../lib/tft-bindings/TftTrait';
import type { TftTrends } from '../lib/tft-bindings/TftTrends';
import type { TftUnit } from '../lib/tft-bindings/TftUnit';

const UNITS: [string, string, number, string[]][] = [
  ['DA_18_Ornn', 'Ornn', 1, ['DA_18_Elderwood', 'DA_18_Vanguard']],
  ['DA_18_Veigar', 'Veigar', 1, ['DA_18_Sprykin', 'DA_18_Spellweaver']],
  ['DA_18_Leona', 'Leona', 1, ['DA_18_Solar', 'DA_18_Defender']],
  ['DA_18_Yorick', 'Yorick', 1, ['DA_18_Blackthorn', 'DA_Juggernaut18']],
  ['DA_18_Varus', 'Varus', 1, ['DA_18_Lunar', 'DA_18_Rapidfire']],
  ['DA_18_Rakan', 'Rakan', 1, ['DA_18_Fae', 'DA_18_Defender']],
  ['DA_Karma18', 'Karma', 1, ['DA_18_Blossom', 'DA_18_Invoker']],
  ['DA_18_Alistar', 'Alistar', 2, ['DA_18_Brawler', 'DA_18_Vanguard']],
  ['DA_18_LeBlanc', 'LeBlanc', 2, ['DA_18_Coven', 'DA_18_Spellweaver']],
  ['DA_18_Sejuani', 'Sejuani', 2, ['DA_18_Elderwood', 'DA_Juggernaut18']],
  ['DA_18_Teemo', 'Teemo', 2, ['DA_18_Sprykin', 'DA_18_Hunter']],
  ['DA_18_Shen', 'Shen', 2, ['DA_18_Lunar', 'DA_18_Defender']],
  ['DA_18_Kayle', 'Kayle', 2, ['DA_18_Solar', 'DA_18_Executioner']],
  ['DA_18_Rammus', 'Rammus', 3, ['DA_18_Brawler', 'DA_18_Defender']],
  ['DA_18_Diana', 'Diana', 3, ['DA_18_Lunar', 'DA_18_Slayer']],
  ['DA_18_Hecarim', 'Hecarim', 3, ['DA_18_Elderwood', 'DA_18_Slayer']],
  ['DA_18_Azir', 'Azir', 3, ['DA_18_Solar', 'DA_18_Summoner']],
  ['DA_18_Cassiopeia', 'Cassiopeia', 3, ['DA_18_Coven', 'DA_18_Invoker']],
  ['DA_Vi18', 'Vi', 3, ['DA_18_Brawler', 'DA_18_Executioner']],
  ['DA_18_Ahri', 'Ahri', 4, ['DA_18_Fae', 'DA_18_Spellweaver']],
  ['DA_18_Ezreal', 'Ezreal', 4, ['DA_18_Hunter', 'DA_18_Rapidfire']],
  ['DA_18_Soraka', 'Soraka', 4, ['DA_18_Blossom', 'DA_18_Invoker']],
  ['DA_18_Zyra', 'Zyra', 4, ['DA_FloraFatalis18', 'DA_18_Summoner']],
  ['DA_18_Morgana', 'Morgana', 4, ['DA_18_Coven', 'DA_18_Spellweaver']],
  ['DA_18_Sett', 'Sett', 4, ['DA_18_Brawler', 'DA_Juggernaut18']],
  ['DA_18_Aphelios', 'Aphelios', 4, ['DA_18_Lunar', 'DA_18_Rapidfire']],
  ['DA_18_Maokai', 'Maokai', 5, ['DA_18_Elderwood', 'DA_18_Greenfather']],
  ['DA_18_Kennen', 'Kennen', 5, ['DA_18_Sprykin', 'DA_18_Invoker']],
  ['DA_Taric18', 'Taric', 5, ['DA_Emerald18', 'DA_18_Defender']],
  ['DA_18_Ivern', 'Ivern', 5, ['DA_18_Elderwood', 'DA_18_Summoner']],
  ['DA_Lux18_Base', 'Lux', 5, ['DA_18_Fae', 'DA_18_LuxUniqueTrait']],
];

const TRAITS: [string, string, number[]][] = [
  ['DA_18_Spellweaver', 'Spellweaver', [2, 4, 6]],
  ['DA_18_Coven', 'Coven', [3]],
  ['DA_18_Invoker', 'Invoker', [2, 4]],
  ['DA_18_Sprykin', 'Sprykin', [2, 4, 6]],
  ['DA_18_Defender', 'Defender', [2, 4, 6]],
  ['DA_18_Brawler', 'Brawler', [2, 4, 6]],
  ['DA_18_Elderwood', 'Elderwood', [3, 5, 7]],
  ['DA_18_Vanguard', 'Vanguard', [2, 4]],
  ['DA_Juggernaut18', 'Juggernaut', [2, 4]],
  ['DA_18_Lunar', 'Lunar', [2, 4]],
  ['DA_18_Solar', 'Solar', [2, 4]],
  ['DA_18_Rapidfire', 'Rapidfire', [2, 4]],
  ['DA_18_Hunter', 'Hunter', [2, 4]],
  ['DA_18_Fae', 'Fae', [2, 3, 5]],
  ['DA_18_Blossom', 'Blossom', [2]],
  ['DA_18_Summoner', 'Summoner', [2, 4]],
  ['DA_18_Slayer', 'Slayer', [2, 4]],
  ['DA_18_Executioner', 'Executioner', [2, 3]],
  ['DA_18_Blackthorn', 'Blackthorn', [1]],
  ['DA_FloraFatalis18', 'Flora Fatalis', [1]],
  ['DA_Emerald18', 'Emerald', [1]],
  ['DA_18_Greenfather', 'Greenfather', [1]],
  ['DA_18_LuxUniqueTrait', 'Lightbringer', [1]],
];

type ItemKind = 'component' | 'craft' | 'radiant' | 'artifact' | 'emblem' | 'special';
const ITEMS: [string, string, ItemKind, string[]][] = [
  ['DA_Component_BFSword', 'B.F. Sword', 'component', []],
  ['DA_Component_RecurveBow', 'Recurve Bow', 'component', []],
  ['DA_Component_NeedlesslyLargeRod', 'Needlessly Large Rod', 'component', []],
  ['DA_Component_TearOfTheGoddess', 'Tear of the Goddess', 'component', []],
  ['DA_Component_ChainVest', 'Chain Vest', 'component', []],
  ['DA_Component_NegatronCloak', 'Negatron Cloak', 'component', []],
  ['DA_Component_GiantsBelt', "Giant's Belt", 'component', []],
  ['DA_Component_SparringGloves', 'Sparring Gloves', 'component', []],
  ['DA_GuinsoosRageblade', "Guinsoo's Rageblade", 'craft', ['DA_Component_RecurveBow', 'DA_Component_NeedlesslyLargeRod']],
  ['DA_JeweledGauntlet', 'Jeweled Gauntlet', 'craft', ['DA_Component_NeedlesslyLargeRod', 'DA_Component_SparringGloves']],
  ['DA_BlueBuff', 'Blue Buff', 'craft', ['DA_Component_TearOfTheGoddess', 'DA_Component_TearOfTheGoddess']],
  ['DA_SpearOfShojin', 'Spear of Shojin', 'craft', ['DA_Component_BFSword', 'DA_Component_TearOfTheGoddess']],
  ['DA_InfinityEdge', 'Infinity Edge', 'craft', ['DA_Component_BFSword', 'DA_Component_SparringGloves']],
  ['DA_RabadonsDeathcap', "Rabadon's Deathcap", 'craft', ['DA_Component_NeedlesslyLargeRod', 'DA_Component_NeedlesslyLargeRod']],
  ['DA_ArchangelsStaff', "Archangel's Staff", 'craft', ['DA_Component_NeedlesslyLargeRod', 'DA_Component_TearOfTheGoddess']],
  ['DA_GargoyleStoneplate', 'Gargoyle Stoneplate', 'craft', ['DA_Component_ChainVest', 'DA_Component_NegatronCloak']],
  ['DA_WarmogsArmor', "Warmog's Armor", 'craft', ['DA_Component_GiantsBelt', 'DA_Component_GiantsBelt']],
  ['DA_SunfireCape', 'Sunfire Cape', 'craft', ['DA_Component_ChainVest', 'DA_Component_GiantsBelt']],
  ['DA_Bloodthirster', 'Bloodthirster', 'craft', ['DA_Component_BFSword', 'DA_Component_NegatronCloak']],
  ['DA_HandOfJustice', 'Hand of Justice', 'craft', ['DA_Component_TearOfTheGoddess', 'DA_Component_SparringGloves']],
  ['DA_JeweledGauntletRadiant', 'Radiant Jeweled Gauntlet', 'radiant', []],
  ['DA_NashorsToothRadiant', "Radiant Nashor's Tooth", 'radiant', []],
  ['DA_Fishbones', 'Fishbones', 'artifact', []],
  ['DA_18_InvokerEmblem', 'Invoker Emblem', 'emblem', []],
  ['TFT_Item_ChonccsCrown', "Choncc's Crown", 'special', []],
];

const AUGMENTS: [string, string, TftAugment['tier'], string][] = [
  ['DA_Augment_JeweledLotus', 'Jeweled Lotus II', 'gold', 'Abilities can critically strike; +15% crit chance.'],
  ['DA_Augment_SpellweaverHeart', 'Spellweaver Heart', 'silver', 'Gain a Spellweaver Emblem-like +1 Spellweaver.'],
  ['DA_Augment_ClimbTheLadder', 'Climb the Ladder', 'gold', 'Gain gold and XP after each player combat.'],
  ['DA_Augment_PandorasItems', "Pandora's Items", 'gold', 'Items on your bench randomize each round.'],
  ['DA_Augment_TinyTitans', 'Tiny Titans', 'silver', 'Your Tactician heals and gains max HP.'],
];

const key = (name: string) => name.replace(/[^A-Za-z]/g, '');
const units = Object.fromEntries(
  UNITS.map(([api, name, cost, traits]): [string, TftUnit] => [
    api,
    { api, name, cost, traits, icon: `ASSETS/Characters/TFT18_${key(name)}/HUD/TFT18_${key(name)}_Square.TFT_Set18.tex` },
  ]),
);
const traits = Object.fromEntries(
  TRAITS.map(([api, name, breakpoints]): [string, TftTrait] => [
    api,
    { api, name, breakpoints, icon: `ASSETS/UX/TraitIcons/Trait_Icon_18_${key(name)}.TFT_Set18.tex` },
  ]),
);
const items = Object.fromEntries(
  ITEMS.map(([api, name, kind, components]): [string, TftItem] => [
    api,
    {
      api,
      name,
      components,
      component: kind === 'component',
      emblem: kind === 'emblem',
      radiant: kind === 'radiant',
      artifact: kind === 'artifact',
      icon: `ASSETS/Maps/TFT/Icons/Items/Hexcore/TFT_Item_${key(api.replace(/^DA_(Component_)?/, ''))}.TFT_Set13.tex`,
    },
  ]),
);
const augments = Object.fromEntries(
  AUGMENTS.map(([api, name, tier, desc]): [string, TftAugment] => [
    api,
    { api, name, tier, desc, icon: `ASSETS/Maps/TFT/Icons/Augments/Hexcore/${key(name)}.TFT_Set18.tex` },
  ]),
);

export const tftSet: TftSetData = { version: '16.21.1', set: 18, units, traits, items, augments };

// Queue ids are bigint in the generated bindings (tft-model `queue: i64`); the backend sends numbers.
const q = (n: number) => n;

const COMPS: Record<string, { name: string; units: string[]; avg: number; levelling: string }> = {
  '426003': {
    name: 'Spellweaver Ahri',
    units: ['DA_18_Ahri', 'DA_18_Morgana', 'DA_18_LeBlanc', 'DA_18_Veigar', 'DA_18_Soraka', 'DA_Karma18', 'DA_18_Cassiopeia', 'DA_18_Kennen'],
    avg: 4.12,
    levelling: 'lvl 8',
  },
  '426011': {
    name: 'Coven Morgana',
    units: ['DA_18_Morgana', 'DA_18_LeBlanc', 'DA_18_Cassiopeia', 'DA_18_Soraka', 'DA_18_Sett', 'DA_18_Rammus', 'DA_18_Leona', 'DA_18_Shen'],
    avg: 4.31,
    levelling: 'lvl 8',
  },
  '426020': {
    name: 'Rapidfire Aphelios',
    units: ['DA_18_Aphelios', 'DA_18_Ezreal', 'DA_18_Varus', 'DA_18_Diana', 'DA_18_Shen', 'DA_18_Rammus', 'DA_Taric18', 'DA_18_Teemo'],
    avg: 4.05,
    levelling: 'lvl 9',
  },
  '426031': {
    name: 'Elderwood Maokai',
    units: ['DA_18_Maokai', 'DA_18_Ivern', 'DA_18_Hecarim', 'DA_18_Sejuani', 'DA_18_Ornn', 'DA_18_Zyra', 'DA_18_Sett', 'DA_Lux18_Base'],
    avg: 3.98,
    levelling: 'lvl 9',
  },
  '426042': {
    name: 'Brawler Vi',
    units: ['DA_Vi18', 'DA_18_Alistar', 'DA_18_Rammus', 'DA_18_Sett', 'DA_18_Kayle', 'DA_18_Yorick', 'DA_18_Teemo'],
    avg: 4.4,
    levelling: 'lvl 7',
  },
  '426055': {
    name: 'Solar Azir',
    units: ['DA_18_Azir', 'DA_18_Leona', 'DA_18_Kayle', 'DA_18_Zyra', 'DA_Taric18', 'DA_18_Ivern', 'DA_18_Rakan'],
    avg: 4.22,
    levelling: 'lvl 8',
  },
  'choncc-kennen-invoker': {
    name: 'Invoker Kennen',
    units: ['DA_18_Kennen', 'DA_18_Soraka', 'DA_Karma18', 'DA_18_Cassiopeia', 'DA_18_Veigar', 'DA_18_Teemo', 'DA_18_Shen', 'DA_18_Ornn'],
    avg: 3.61,
    levelling: 'lvl 8',
  },
  'choncc-kennen-sprykin': {
    name: 'Sprykin Kennen',
    units: ['DA_18_Kennen', 'DA_18_Veigar', 'DA_18_Teemo', 'DA_18_Rammus', 'DA_18_Shen', 'DA_18_Leona', 'DA_18_Ahri'],
    avg: 3.9,
    levelling: 'lvl 7',
  },
};

/** A comp pick with what I have of it; `have` empty = lobby (nothing owned). */
function pick(comp: string, have: string[] = []): CompPick {
  const c = COMPS[comp];
  const missing = c.units.filter((u) => !have.includes(u));
  return {
    comp,
    name: c.name,
    fit: have.length / c.units.length,
    avg_place: c.avg,
    have: c.units.filter((u) => have.includes(u)),
    missing,
    levelling: c.levelling,
  };
}

const HOUR = 3_600_000;
const now = Date.now();
let gameId = 7_412_000_000;
function game(
  hoursAgo: number,
  queue: number,
  placement: number,
  level: number,
  comp: string | null,
  board: [string, number, string[]][],
): TftPastGame {
  const counts = new Map<string, number>();
  for (const [u] of board) for (const t of units[u]?.traits ?? []) counts.set(t, (counts.get(t) ?? 0) + 1);
  return {
    game_id: gameId++,
    created: now - hoursAgo * HOUR,
    queue: q(queue),
    mode: queue === 1230 ? 'choncc' : 'standard',
    placement,
    level,
    last_round: 24 + level * 2 - placement,
    duration_s: 1500 + (8 - placement) * 140,
    units: board.map(([api, star, its]) => ({ api, star, items: its })),
    traits: [...counts].map(([t, n]): [string, number, number] => [t, n, (traits[t]?.breakpoints ?? []).filter((b) => n >= b).length]),
    comp,
  };
}

const history: TftPastGame[] = [
  game(1, 1100, 3, 8, '426003', [
    ['DA_18_Ahri', 2, ['DA_JeweledGauntlet', 'DA_BlueBuff', 'DA_RabadonsDeathcap']],
    ['DA_18_Morgana', 2, ['DA_ArchangelsStaff']],
    ['DA_18_LeBlanc', 2, []],
    ['DA_18_Veigar', 3, []],
    ['DA_18_Soraka', 1, []],
    ['DA_Karma18', 2, []],
    ['DA_18_Rammus', 2, ['DA_GargoyleStoneplate', 'DA_WarmogsArmor']],
    ['DA_18_Shen', 2, ['DA_SunfireCape']],
  ]),
  game(2, 1100, 6, 8, '426011', [
    ['DA_18_Morgana', 2, ['DA_SpearOfShojin', 'DA_JeweledGauntlet']],
    ['DA_18_Cassiopeia', 2, ['DA_BlueBuff']],
    ['DA_18_LeBlanc', 2, []],
    ['DA_18_Sett', 1, ['DA_WarmogsArmor']],
    ['DA_18_Leona', 2, []],
    ['DA_18_Shen', 2, []],
    ['DA_18_Rammus', 1, []],
  ]),
  game(5, 1230, 2, 9, 'choncc-kennen-invoker', [
    ['DA_18_Kennen', 2, ['DA_JeweledGauntletRadiant', 'DA_SpearOfShojin', 'DA_RabadonsDeathcap']],
    ['DA_18_Soraka', 2, ['DA_NashorsToothRadiant']],
    ['DA_Karma18', 3, []],
    ['DA_18_Cassiopeia', 2, []],
    ['DA_18_Veigar', 3, []],
    ['DA_18_Teemo', 2, []],
    ['DA_18_Shen', 2, ['DA_GargoyleStoneplate']],
    ['DA_18_Ornn', 2, ['DA_WarmogsArmor']],
    ['DA_Taric18', 1, []],
  ]),
  game(26, 1100, 1, 9, '426031', [
    ['DA_18_Maokai', 2, ['DA_WarmogsArmor', 'DA_GargoyleStoneplate']],
    ['DA_18_Ivern', 2, ['DA_ArchangelsStaff', 'DA_BlueBuff']],
    ['DA_18_Hecarim', 2, ['DA_Bloodthirster']],
    ['DA_18_Sejuani', 2, []],
    ['DA_18_Ornn', 2, []],
    ['DA_18_Zyra', 2, []],
    ['DA_18_Sett', 1, []],
    ['DA_Lux18_Base', 1, []],
    ['DA_Taric18', 1, []],
  ]),
  game(28, 1100, 7, 7, '426042', [
    ['DA_Vi18', 2, ['DA_InfinityEdge', 'DA_Bloodthirster']],
    ['DA_18_Alistar', 2, []],
    ['DA_18_Rammus', 2, ['DA_SunfireCape']],
    ['DA_18_Kayle', 2, []],
    ['DA_18_Yorick', 2, []],
    ['DA_18_Teemo', 1, []],
    ['DA_18_Sett', 1, []],
  ]),
  game(50, 1230, 5, 8, 'choncc-kennen-sprykin', [
    ['DA_18_Kennen', 1, ['DA_JeweledGauntlet', 'DA_Fishbones']],
    ['DA_18_Veigar', 2, []],
    ['DA_18_Teemo', 2, ['DA_GuinsoosRageblade']],
    ['DA_18_Rammus', 2, []],
    ['DA_18_Shen', 2, []],
    ['DA_18_Leona', 2, []],
  ]),
  game(52, 1100, 4, 8, null, [
    ['DA_18_Ezreal', 2, ['DA_InfinityEdge', 'DA_GuinsoosRageblade']],
    ['DA_18_Varus', 2, []],
    ['DA_18_Diana', 2, ['DA_Bloodthirster']],
    ['DA_18_Shen', 2, []],
    ['DA_18_Teemo', 2, []],
    ['DA_18_Rammus', 2, ['DA_GargoyleStoneplate']],
  ]),
];

const trends: TftTrends[] = [
  {
    mode: 'standard',
    games: 14,
    avg_place: 4.21,
    top4: 0.57,
    comps: [
      { comp: '426003', name: 'Spellweaver Ahri', games: 5, avg_place: 3.4, top4: 0.8 },
      { comp: '426011', name: 'Coven Morgana', games: 4, avg_place: 4.75, top4: 0.5 },
      { comp: '426031', name: 'Elderwood Maokai', games: 3, avg_place: 3.67, top4: 0.67 },
      { comp: '426042', name: 'Brawler Vi', games: 2, avg_place: 6.5, top4: 0 },
    ],
  },
  {
    mode: 'choncc',
    games: 5,
    avg_place: 3.8,
    top4: 0.6,
    comps: [
      { comp: 'choncc-kennen-invoker', name: 'Invoker Kennen', games: 3, avg_place: 3, top4: 0.67 },
      { comp: 'choncc-kennen-sprykin', name: 'Sprykin Kennen', games: 2, avg_place: 5, top4: 0.5 },
    ],
  },
];

const META = ['426031', '426020', '426003', '426055', '426011', '426042'];
const status = (screen: boolean): [string, boolean][] => [
  ['Set data', true],
  ['Meta', true],
  ['Screen reading', screen],
];

const standardAdvice: TftAdvice = {
  mode: 'standard',
  stage: [3, 2],
  owned: [
    { api: 'DA_18_LeBlanc', copies: 3, star: 2, board: 1, bench: 0 },
    { api: 'DA_18_Veigar', copies: 4, star: 2, board: 1, bench: 0 },
    { api: 'DA_Karma18', copies: 2, star: 1, board: 1, bench: 1 },
    { api: 'DA_18_Cassiopeia', copies: 1, star: 1, board: 1, bench: 0 },
    { api: 'DA_18_Ahri', copies: 1, star: 1, board: 1, bench: 0 },
    { api: 'DA_18_Leona', copies: 3, star: 2, board: 1, bench: 0 },
    { api: 'DA_18_Shen', copies: 1, star: 1, board: 0, bench: 1 },
    { api: '', copies: 1, star: 1, board: 0, bench: 1 },
  ],
  comps: [
    pick('426003', ['DA_18_LeBlanc', 'DA_18_Veigar', 'DA_Karma18', 'DA_18_Cassiopeia', 'DA_18_Ahri']),
    pick('426011', ['DA_18_LeBlanc', 'DA_18_Cassiopeia', 'DA_18_Leona', 'DA_18_Shen']),
    pick('426020', ['DA_18_Shen']),
  ],
  shop: [
    { slot: 0, unit: 'DA_18_Ahri', buy: true, reason: 'Second Ahri: your carry, 4 held by others, 7 left', contested: 2, held: 4, left: 7 },
    { slot: 1, unit: 'DA_18_Rammus', buy: false, reason: 'Not in Spellweaver Ahri', contested: 0, held: 0, left: null },
    { slot: 2, unit: 'DA_18_Morgana', buy: true, reason: 'For Spellweaver Ahri (4 Spellweaver)', contested: 0, held: 0, left: null },
    { slot: 3, unit: 'DA_Karma18', buy: true, reason: 'Makes ★2 Karma', contested: 0, held: 0, left: null },
    { slot: 4, unit: 'DA_18_Varus', buy: false, reason: 'Not in your comp; sell value only', contested: 1, held: 2, left: 16 },
  ],
  econ: { action: 'level', text: 'Level to 6 now (3-2): 34 gold, keep 30 for interest' },
  items: [
    {
      item: 'DA_JeweledGauntlet',
      from: ['DA_Component_NeedlesslyLargeRod', 'DA_Component_SparringGloves'],
      holder: 'DA_18_Ahri',
      now: true,
      reason: 'HP 46 and falling: slam it for the 3-2 fights',
    },
    {
      item: 'DA_BlueBuff',
      from: ['DA_Component_TearOfTheGoddess', 'DA_Component_TearOfTheGoddess'],
      holder: 'DA_18_Ahri',
      now: false,
      reason: 'Second Ahri item; hold the Tears until 4-1',
    },
  ],
  augments: [
    { augment: 'DA_Augment_JeweledLotus', score: 0.82, reason: 'Ahri and Morgana crit with spells; avg 3.9 in this comp' },
    { augment: 'DA_Augment_SpellweaverHeart', score: 0.74, reason: '+1 Spellweaver: 4 → 6 with Morgana' },
    { augment: 'DA_Augment_ClimbTheLadder', score: 0.41, reason: 'Econ; weak while you are losing HP' },
  ],
  contested: [
    { unit: 'DA_18_Ahri', boards: 2, held: 4, left: 7 },
    { unit: 'DA_18_Kennen', boards: 1, held: 0, left: 16 },
  ],
  calls: [
    { key: 'aug-3-2', text: 'Take Jeweled Lotus II', say: 'Take Jeweled Lotus' },
    { key: 'contested-DA_18_Ahri', text: 'Ahri: 4 held by others, 7 left: consider a pivot from Spellweaver Ahri', say: 'Ahri is contested: 7 left.' },
  ],
  choncc: null,
};

const chonccAdvice: TftAdvice = {
  mode: 'choncc',
  stage: [4, 1],
  owned: [
    { api: 'DA_18_Kennen', copies: 1, star: 1, board: 1, bench: 0 },
    { api: 'DA_18_Veigar', copies: 6, star: 2, board: 1, bench: 0 },
    { api: 'DA_18_Teemo', copies: 3, star: 2, board: 1, bench: 0 },
    { api: 'DA_Karma18', copies: 4, star: 2, board: 1, bench: 0 },
    { api: 'DA_18_Soraka', copies: 1, star: 1, board: 1, bench: 0 },
    { api: 'DA_18_Cassiopeia', copies: 3, star: 2, board: 1, bench: 0 },
    { api: 'DA_18_Ornn', copies: 2, star: 1, board: 1, bench: 0 },
    { api: 'DA_18_Shen', copies: 3, star: 2, board: 1, bench: 0 },
  ],
  comps: [
    pick('choncc-kennen-invoker', ['DA_18_Kennen', 'DA_18_Soraka', 'DA_Karma18', 'DA_18_Cassiopeia', 'DA_18_Veigar', 'DA_18_Teemo', 'DA_18_Shen']),
    pick('choncc-kennen-sprykin', ['DA_18_Kennen', 'DA_18_Veigar', 'DA_18_Teemo', 'DA_18_Shen']),
    pick('426003', ['DA_18_Veigar', 'DA_Karma18', 'DA_18_Cassiopeia', 'DA_18_Soraka', 'DA_18_Kennen']),
  ],
  shop: [
    { slot: 0, unit: 'DA_18_Kennen', buy: true, reason: 'Second Kennen: your starter carry', contested: 0, held: 0, left: null },
    { slot: 1, unit: 'DA_18_Soraka', buy: true, reason: 'For Invoker Kennen (4 Invoker), contested by 1 board', contested: 1, held: 0, left: 24 },
    { slot: 2, unit: 'DA_18_Hecarim', buy: false, reason: 'Not in Invoker Kennen', contested: 0, held: 0, left: null },
    { slot: 4, unit: 'DA_18_Ornn', buy: true, reason: 'Makes ★2 Ornn (frontline)', contested: 0, held: 0, left: null },
  ],
  econ: { action: 'roll', text: "Roll down to 10 gold for ★2 Kennen: 9 slots with Choncc's Crown" },
  items: [
    {
      item: 'DA_JeweledGauntletRadiant',
      from: [],
      holder: 'DA_18_Kennen',
      now: true,
      reason: 'Radiant from the Golden Orb: put it on Kennen now',
    },
    {
      item: 'DA_SpearOfShojin',
      from: ['DA_Component_BFSword', 'DA_Component_TearOfTheGoddess'],
      holder: 'DA_18_Kennen',
      now: false,
      reason: "Kennen's second item once a Tear drops",
    },
  ],
  augments: [],
  contested: [{ unit: 'DA_18_Soraka', boards: 1, held: 0, left: 24 }],
  calls: [{ key: 'roll-4-1', text: 'Roll down now', say: 'Roll down now' }],
  choncc: {
    starter: 'DA_18_Kennen',
    comps: [
      pick('choncc-kennen-invoker', ['DA_18_Kennen', 'DA_18_Soraka', 'DA_Karma18', 'DA_18_Cassiopeia', 'DA_18_Veigar', 'DA_18_Teemo', 'DA_18_Shen']),
      pick('choncc-kennen-sprykin', ['DA_18_Kennen', 'DA_18_Veigar', 'DA_18_Teemo', 'DA_18_Shen']),
    ],
    loot: [
      'Radiant Jeweled Gauntlet → Kennen',
      "Choncc's Crown: +2 team size before 4-5",
      'Invoker Emblem → Soraka (Invoker 6)',
      'Components: Tear first, then Rod',
    ],
    notes: [
      'Team size 9/9 read from the board (Crown +2).',
      'Under 50 HP the Golden Orb drops a radiant item, a reforger and gold.',
    ],
  },
};

const base = { history, trends, review: null, advice: null, meta: [] as CompPick[] };

export const tftNone: TftState = { ...base, phase: 'none', queue: null, mode: 'standard', history: [], trends: [], status: [] };

export const tftFixtures = {
  'tft ingame': { ...base, phase: 'ingame', queue: q(1100), mode: 'standard', advice: standardAdvice, status: status(true) },
  'tft ingame choncc': { ...base, phase: 'ingame', queue: q(1230), mode: 'choncc', advice: chonccAdvice, status: status(true) },
  'tft lobby': { ...base, phase: 'lobby', queue: q(1100), mode: 'standard', meta: META.map((c) => pick(c)), status: status(false) },
  'tft lobby choncc': {
    ...base,
    phase: 'lobby',
    queue: q(1230),
    mode: 'choncc',
    meta: ['choncc-kennen-invoker', ...META.slice(0, 4), 'choncc-kennen-sprykin'].map((c) => pick(c)),
    status: status(false),
  },
  'tft postgame': {
    ...base,
    phase: 'postgame',
    queue: q(1100),
    mode: 'standard',
    review: {
      game: history[0],
      comp: pick('426003'),
      takeaways: [
        'Spellweaver Ahri averages 4.12; you placed 3rd.',
        "Ahri held Jeweled Gauntlet + Blue Buff + Rabadon's; the best build swaps Rabadon's for Spear of Shojin (avg 3.6).",
        'Reached level 8 at 4-5; the comp levels to 8 at 4-2.',
      ],
    },
    status: status(false),
  },
} satisfies Record<string, TftState>;

export type TftFixtureName = keyof typeof tftFixtures;
