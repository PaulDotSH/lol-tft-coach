// Self-coaching goal metadata: labels, units, defaults and formatting.
import type { GoalKind } from './bindings/GoalKind';
import type { GoalProgress } from './bindings/GoalProgress';

export interface GoalMeta {
  label: string;
  /** Lower values are better. */
  lower: boolean;
  target: number;
  step: number;
  digits: number;
  unit: string;
  riftOnly: boolean;
}

export const GOALS: Record<GoalKind, GoalMeta> = {
  cs10: { label: 'CS/min at 10:00', lower: false, target: 7, step: 0.5, digits: 1, unit: '/min', riftOnly: true },
  earlydeaths: { label: 'Deaths before 14:00', lower: true, target: 1, step: 1, digits: 0, unit: '', riftOnly: true },
  vision: { label: 'Vision per minute', lower: false, target: 1, step: 0.1, digits: 2, unit: '/min', riftOnly: true },
  unspent: { label: 'Gold held at death', lower: true, target: 1000, step: 100, digits: 0, unit: ' g', riftOnly: false },
  buynext: { label: 'Followed Buy next', lower: false, target: 70, step: 5, digits: 0, unit: '%', riftOnly: false },
};

export const GOAL_KINDS = Object.keys(GOALS) as GoalKind[];
export const MAX_GOALS = 2;

export const fmtGoal = (kind: GoalKind, v: number) => `${v.toFixed(GOALS[kind].digits)}${GOALS[kind].unit}`;

/** Target as a sentence fragment: "≥ 7.0/min", "≤ 1", "under 1000 g". */
export const targetText = (kind: GoalKind, t: number) =>
  kind === 'unspent' ? `under ${fmtGoal(kind, t)}` : `${GOALS[kind].lower ? '≤' : '≥'} ${fmtGoal(kind, t)}`;

/** Current value of a progress entry. */
export function progressText(p: GoalProgress, live: boolean): string {
  if (p.value == null) return '—';
  if (p.kind === 'unspent') return p.value === 0 ? 'no deaths holding it' : `${p.value}× at ${fmtGoal('unspent', p.target)}+`;
  if (p.kind === 'cs10' && live && p.met == null) return `${fmtGoal('cs10', p.value)} pace`;
  return fmtGoal(p.kind, p.value);
}
