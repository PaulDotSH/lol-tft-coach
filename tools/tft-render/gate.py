"""T8 gate: name the labelled real crops with the seed looks of every rendered unit and sweep the
naming threshold. A crop is named when its best unit's score is at least `sim` and leads the
next unit by `margin`; it is wrong when named but not its label ("-" crops, no unit visible, are
wrong whenever named). Reports, per group and scoring rule, the most crops named with at most
5 % of all crops named wrong, plus top-1/top-3.
Rules: `max` (best look per unit, as Library::classify), `top5` (mean of a unit's 5 best looks),
each raw or `centred` (both sides less their mean: the seed's, and the group's real crops').
Usage: py -3.12 -I tools/tft-render/gate.py <dump dir>... [--wrong 0.05]
"""
import os
import sys
from collections import defaultdict

import numpy as np

sys.path.insert(0, os.path.dirname(__file__))
import calibrate as C  # noqa: E402

LABELS = C.LABELS


def norm(x):
    return x / np.linalg.norm(x, axis=-1, keepdims=True)


def crops():
    """(group, label, embedding): labelled crops, "-" kept as negatives."""
    import json
    out = [(g, a, e) for g, a, e in C.real_crops()]
    for l in open(LABELS, encoding='utf8'):
        j = json.loads(l)
        if j['api'] == '-':
            f = os.path.join(C.EVAL, j['game'], j['group'], f"{j['frame']}_{j['x']:.0f}_{j['y']:.0f}.f32")
            if os.path.exists(f):
                out.append((j['group'], '-', np.fromfile(f, '<f4')))
    return out


def scores(E, idx, units, e, rule):
    if rule == 'max':
        return np.array([(E[idx[u]] @ e).max() for u in units])
    return np.array([np.sort(E[idx[u]] @ e)[-5:].mean() for u in units])


def main():
    wrong_cap = float(sys.argv[sys.argv.index('--wrong') + 1]) if '--wrong' in sys.argv else 0.05
    dirs = [a for a in sys.argv[1:] if not a.startswith('--') and a != str(wrong_cap)]
    parts = [C.load_dump(d) for d in dirs]
    E = np.concatenate([p[0] for p in parts])
    meta = sum([p[1] for p in parts], [])
    units = sorted({m['api'] for m in meta})
    idx = {u: np.array([i for i, m in enumerate(meta) if m['api'] == u]) for u in units}
    real = [c for c in crops() if c[1] == '-' or c[1] in idx]
    print(f'{len(units)} rendered units, {len(E)} looks; crops: ' + ', '.join(f'{g} {sum(1 for c in real if c[0] == g)}' for g in ('combat', 'mine')))
    for centred in (False, True):
        Ec = norm(E - E.mean(0)) if centred else E
        mu = {g: np.mean([e for gg, a, e in real if gg == g], 0) for g in ('combat', 'mine')}
        for rule in ('max', 'top5'):
            res = defaultdict(list)
            for g, a, e in real:
                e2 = norm(e - mu[g]) if centred else e
                s = scores(Ec, idx, units, e2, rule)
                o = np.argsort(-s)
                res[g].append((a, units[o[0]], float(s[o[0]]), float(s[o[0]] - s[o[1]]), a in [units[k] for k in o[:3]]))
            for g, rows in sorted(res.items()):
                n = len(rows)
                pos = [r for r in rows if r[0] != '-']
                t1 = sum(r[0] == r[1] for r in pos) / max(len(pos), 1)
                t3 = sum(r[4] for r in pos) / max(len(pos), 1)
                best = (0.0, None, 0.0)
                sims = sorted({round(r[2], 3) for r in rows})
                for t in sims[:: max(1, len(sims) // 60)]:
                    for m in (0.0, 0.01, 0.02, 0.03, 0.05, 0.08, 0.12):
                        named = [r for r in rows if r[2] >= t and r[3] >= m]
                        wrong = sum(r[0] != r[1] for r in named)
                        if wrong <= wrong_cap * n and (len(named) - wrong) / n > best[0]:
                            best = ((len(named) - wrong) / n, (t, m), wrong / n)
                print(f"{'centred' if centred else 'raw':8} {rule:4} {g:6} n={n} top1={t1:.2f} top3={t3:.2f}  "
                      f"named right={best[0]:.2f} wrong={best[2]:.2f} at sim/margin={best[1]}")


if __name__ == '__main__':
    main()
