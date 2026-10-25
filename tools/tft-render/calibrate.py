"""Score seed_looks dumps against the labelled real crops (T5): for each composite setting
(pitch = dump dir, height h, bar gap, dx), how often a real crop's best-matching rendered unit is
its label (top-1 / top-3), split by group (mine, combat). Only units both labelled and rendered
count. Usage: py -3.12 -I tools/tft-render/calibrate.py <dump dir>... [--json out.json]
"""
import json
import os
import sys
from collections import defaultdict

import numpy as np

DIM = 768
EVAL = os.path.expandvars(r'%LOCALAPPDATA%\lol-coach\dev\render\eval')
LABELS = os.path.join(os.path.dirname(__file__), 'eval', 'labels.jsonl')


def real_crops():
    """(group, api, embedding) of every labelled crop."""
    out = []
    for l in open(LABELS, encoding='utf8'):
        j = json.loads(l)
        if j['api'] in ('?', '-'):
            continue
        f = os.path.join(EVAL, j['game'], j['group'], f"{j['frame']}_{j['x']:.0f}_{j['y']:.0f}.f32")
        if os.path.exists(f):
            out.append((j['group'], j['api'], np.fromfile(f, '<f4')))
    for game in sorted(os.listdir(EVAL)):
        idx = os.path.join(EVAL, game, 'crops.jsonl')
        if not os.path.exists(idx):
            continue
        for l in open(idx, encoding='utf8'):
            j = json.loads(l)
            if j['group'] == 'mine' and j.get('api'):
                out.append(('mine', j['api'], np.fromfile(os.path.join(EVAL, game, j['file'][:-4] + '.f32'), '<f4')))
    return out


def load_dump(d):
    e = np.fromfile(os.path.join(d, 'embs.f32'), '<f4').reshape(-1, DIM)
    meta = [json.loads(l) for l in open(os.path.join(d, 'index.jsonl'), encoding='utf8')]
    return e, meta


def score(real, embs, meta, keep):
    """Top-1 / top-3 per group over the looks `keep` selects."""
    by_unit = defaultdict(list)
    for i, m in enumerate(meta):
        if keep(m):
            by_unit[m['api']].append(i)
    units = sorted(by_unit)
    res = defaultdict(lambda: [0, 0, 0])
    for group, api, e in real:
        if api not in by_unit:
            continue
        sims = np.array([float((embs[by_unit[u]] @ e).max()) for u in units])
        order = [units[k] for k in np.argsort(-sims)]
        r = res[group]
        r[0] += 1
        r[1] += order[0] == api
        r[2] += api in order[:3]
    return {g: (n, t1 / n, t3 / n) for g, (n, t1, t3) in res.items()}


def main():
    args = [a for a in sys.argv[1:] if not a.startswith('--')]
    real = real_crops()
    print(f'{len(real)} labelled crops')
    rows = []
    for d in args:
        embs, meta = load_dump(d)
        keys = sorted({(m['h'], m['gap'], m['dx']) for m in meta})
        for k in keys:
            s = score(real, embs, meta, lambda m, k=k: (m['h'], m['gap'], m['dx']) == k)
            rows.append((os.path.basename(d.rstrip('/\\')), k, s))
        s = score(real, embs, meta, lambda m: True)
        rows.append((os.path.basename(d.rstrip('/\\')), 'all', s))
    for d, k, s in sorted(rows, key=lambda r: -sum(v[1] * v[0] for v in r[2].values())):
        print(d, k, '  '.join(f'{g}: n={n} top1={t1:.2f} top3={t3:.2f}' for g, (n, t1, t3) in sorted(s.items())))
    if '--json' in sys.argv:
        json.dump([[d, str(k), s] for d, k, s in rows], open(sys.argv[sys.argv.index('--json') + 1], 'w'), indent=1)


if __name__ == '__main__':
    main()
