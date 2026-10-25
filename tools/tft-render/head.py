"""Approach 2: a classifier head on DINOv2 features of the composites (seed_looks dumps), scored on
the same gate as gate.py. Domain alignment without labels: each side (composites, real crops)
centred or standardised by its own statistics; the real side's come from every real crop in the
eval dirs, labelled or not. Usage: py -3.12 -I tools/tft-render/head.py <dump dir>... [--save head.npz]
"""
import glob
import os
import sys
from collections import defaultdict

import numpy as np
import torch

sys.path.insert(0, os.path.dirname(__file__))
import calibrate as C  # noqa: E402
import gate as G  # noqa: E402

WRONG = 0.05


def real_pool():
    """Every real crop embedding (labelled or not): the real side's statistics."""
    fs = glob.glob(os.path.join(C.EVAL, '*', '*', '*.f32'))
    return np.stack([np.fromfile(f, '<f4') for f in fs])


def align(x, stats, mode):
    mu, sd = stats
    if mode == 'none':
        return x
    if mode == 'centre':
        return G.norm(x - mu)
    return (x - mu) / sd


def train(X, y, n, hidden, epochs=30, wd=1e-4, drop=0.2, noise=0.0):
    torch.manual_seed(0)
    layers = [torch.nn.Dropout(drop)]
    if hidden:
        layers += [torch.nn.Linear(X.shape[1], hidden), torch.nn.GELU(), torch.nn.Dropout(drop), torch.nn.Linear(hidden, n)]
    else:
        layers += [torch.nn.Linear(X.shape[1], n)]
    m = torch.nn.Sequential(*layers)
    opt = torch.optim.AdamW(m.parameters(), lr=1e-3, weight_decay=wd)
    Xt, yt = torch.tensor(X, dtype=torch.float32), torch.tensor(y)
    for _ in range(epochs):
        perm = torch.randperm(len(Xt))
        for i in range(0, len(Xt), 512):
            b = perm[i:i + 512]
            xb = Xt[b] + noise * torch.randn_like(Xt[b]) if noise else Xt[b]
            loss = torch.nn.functional.cross_entropy(m(xb), yt[b], label_smoothing=0.1)
            opt.zero_grad()
            loss.backward()
            opt.step()
    m.eval()
    return m


def gate(m, real, units):
    res = defaultdict(list)
    with torch.no_grad():
        for g, a, e in real:
            p = torch.softmax(m(torch.tensor(e[None], dtype=torch.float32)), -1)[0].numpy()
            o = np.argsort(-p)
            res[g].append((a, units[o[0]], float(p[o[0]]), float(p[o[0]] - p[o[1]]), a in [units[k] for k in o[:3]]))
    out = {}
    for g, rows in res.items():
        n = len(rows)
        pos = [r for r in rows if r[0] != '-']
        t1 = sum(r[0] == r[1] for r in pos) / len(pos)
        t3 = sum(r[4] for r in pos) / len(pos)
        best = (0.0, None, 0.0)
        for t in np.linspace(0.0, 0.95, 39):
            for mg in (0.0, 0.05, 0.1, 0.2, 0.3):
                named = [r for r in rows if r[2] >= t and r[3] >= mg]
                wrong = sum(r[0] != r[1] for r in named)
                if wrong <= WRONG * n and (len(named) - wrong) / n > best[0]:
                    best = ((len(named) - wrong) / n, (round(float(t), 3), mg), wrong / n)
        out[g] = (n, t1, t3, best)
    return out


def main():
    save = sys.argv[sys.argv.index('--save') + 1] if '--save' in sys.argv else None
    dirs = [a for a in sys.argv[1:] if not a.startswith('--') and a != save]
    parts = [C.load_dump(d) for d in dirs]
    X = np.concatenate([p[0] for p in parts])
    meta = sum([p[1] for p in parts], [])
    units = sorted({m['api'] for m in meta})
    y = np.array([units.index(m['api']) for m in meta])
    pool = real_pool()
    real0 = [c for c in G.crops() if c[1] == '-' or c[1] in units]
    print(f'{len(units)} units, {len(X)} composites, {len(pool)} real crops for statistics, {len(real0)} labelled')
    stats_x = (X.mean(0), X.std(0) + 1e-6)
    stats_r = (pool.mean(0), pool.std(0) + 1e-6)
    best = None
    for mode in ('none', 'centre', 'standard'):
        Xa = align(X, stats_x, mode)
        real = [(g, a, align(e[None], stats_r, mode)[0]) for g, a, e in real0]
        for hidden in (0, 512):
            for noise in (0.0, 0.5):
                m = train(Xa, y, len(units), hidden, noise=noise if mode == 'standard' else noise * 0.05)
                r = gate(m, real, units)
                line = '  '.join(f'{g}: top1={t1:.2f} top3={t3:.2f} named={b[0]:.2f} wrong={b[2]:.2f} at {b[1]}' for g, (n, t1, t3, b) in sorted(r.items()))
                print(f'{mode:8} hidden={hidden:<4} noise={noise}  {line}')
                score = r['combat'][3][0]
                if best is None or score > best[0]:
                    best = (score, mode, hidden, noise, m)
    print('best', best[:4])
    if save:
        m = best[4]
        np.savez(save, units=np.array(units), mode=best[1], mu_x=stats_x[0], sd_x=stats_x[1], mu_r=stats_r[0], sd_r=stats_r[1],
                 **{f'p{i}': p.detach().numpy() for i, p in enumerate(m.parameters())})


if __name__ == '__main__':
    main()
