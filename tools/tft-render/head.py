"""Approach 2: a classifier head on DINOv2 features of the composites (seed_looks dumps), scored on
the same gate as gate.py. Domain alignment without labels: each side (composites, real crops)
centred or standardised by its own statistics; the real side's come from every real crop in the
eval dirs, labelled or not. Usage: py -3.12 -I tools/tft-render/head.py <dump dir>... [--save head.npz]
[--export head.bin] (the best head for the app, TFH1, with its naming thresholds; parity cases beside it).
`--tiny-fixture <dir>`: a small random head and its parity cases for the Rust test, no dumps needed.
"""
import glob
import json
import os
import struct
import sys
from collections import defaultdict

import numpy as np
import torch

sys.path.insert(0, os.path.dirname(__file__))
import calibrate as C  # noqa: E402
import gate as G  # noqa: E402

WRONG = 0.05
MAGIC = b'TFH1'
REJECT = '-'
torch.set_num_threads(int(os.environ.get('TORCH_THREADS', '2')))


def linears(m):
    """The head's Linear layers (one: linear head; two: MLP with GELU between)."""
    return [l for l in m if isinstance(l, torch.nn.Linear)]


def export(m, units, sim, margin, path):
    """TFH1 for `Library::with_head` (crates/tft-coach/src/head.rs): sizes, thresholds, names, f16 weights."""
    ls = linears(m)
    d = ls[0].in_features
    hidden = ls[0].out_features if len(ls) == 2 else 0
    b = bytearray(MAGIC)
    b += struct.pack('<HHHff', d, hidden, len(units), sim, margin)
    for u in units:
        e = u.encode('utf8')
        b += struct.pack('<H', len(e)) + e
    parts = [ls[0].weight, ls[0].bias, ls[1].weight, ls[1].bias] if hidden else [ls[0].weight, ls[0].bias]
    if not hidden:
        parts = [torch.zeros(0), torch.zeros(0)] + parts
    for p in parts:
        b += p.detach().numpy().astype('<f2').tobytes()
    with open(path, 'wb') as f:
        f.write(bytes(b))


def read_head(path):
    """(d, hidden, units, sim, margin, [w1, b1, w2, b2]) of a TFH1 file, weights as float32."""
    with open(path, 'rb') as f:
        b = f.read()
    assert b[:4] == MAGIC
    d, hidden, n, sim, margin = struct.unpack_from('<HHHff', b, 4)
    o, units = 18, []
    for _ in range(n):
        (k,) = struct.unpack_from('<H', b, o)
        units.append(b[o + 2:o + 2 + k].decode('utf8'))
        o += 2 + k
    into = hidden or d
    out = []
    for size in (hidden * d, hidden, n * into, n):
        out.append(np.frombuffer(b, '<f2', size, o).astype(np.float32))
        o += 2 * size
    assert o == len(b)
    return d, hidden, units, sim, margin, out


def parity(m, units, X, path):
    """Cases for the Rust head test: inputs with clear top-1 (lead ≥ 0.02), their top-1 and probability from torch."""
    with torch.no_grad():
        p = torch.softmax(m(torch.tensor(X, dtype=torch.float32)), -1).numpy()
    o = np.argsort(-p, 1)
    keep = [i for i in range(len(X)) if p[i, o[i, 0]] - p[i, o[i, 1]] >= 0.02]
    cases = [{'input': [round(float(v), 6) for v in X[i]], 'top1': units[o[i, 0]], 'score': round(float(p[i, o[i, 0]]), 6)} for i in keep]
    with open(path, 'w', encoding='utf8') as f:
        json.dump(cases, f)
    return len(cases)


def tiny(d=12, hidden=8, units=('DA_18_Ahri', 'DA_18_Sett', 'DA_18_Lux', REJECT), seed=0):
    torch.manual_seed(seed)
    m = torch.nn.Sequential(torch.nn.Dropout(0.0), torch.nn.Linear(d, hidden), torch.nn.GELU(), torch.nn.Dropout(0.0), torch.nn.Linear(hidden, len(units)))
    with torch.no_grad():
        for p in m.parameters():
            p.mul_(3.0)
    m.eval()
    return m, list(units)


def tiny_fixture(out):
    m, units = tiny()
    export(m, units, 0.6, 0.1, os.path.join(out, 'head_tiny.bin'))
    X = np.random.default_rng(1).normal(size=(120, 12)).astype(np.float32)
    n = parity(m, units, X, os.path.join(out, 'head_tiny_parity.json'))
    print(f'tiny head: {n} parity cases in {out}')


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
    if '--tiny-fixture' in sys.argv:
        return tiny_fixture(sys.argv[sys.argv.index('--tiny-fixture') + 1])
    save = sys.argv[sys.argv.index('--save') + 1] if '--save' in sys.argv else None
    exp = sys.argv[sys.argv.index('--export') + 1] if '--export' in sys.argv else None
    dirs = [a for a in sys.argv[1:] if not a.startswith('--') and a not in (save, exp)]
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
                    best = (score, mode, hidden, noise, m, r['combat'][3][1])
    print('best', best[:4])
    if exp:
        if best[1] != 'none':
            print('export: only heads without alignment run in the app; best aligned head not exported')
        else:
            sim, margin = best[5] or (1.0, 1.0)
            export(best[4], units, float(sim), float(margin), exp)
            n = parity(best[4], units, np.stack([e for _, _, e in real0[:100]]), os.path.splitext(exp)[0] + '_parity.json')
            print(f'exported {exp} (sim {sim}, margin {margin}), {n} parity cases')
    if save:
        m = best[4]
        np.savez(save, units=np.array(units), mode=best[1], mu_x=stats_x[0], sd_x=stats_x[1], mu_r=stats_r[0], sd_r=stats_r[1],
                 **{f'p{i}': p.detach().numpy() for i, p in enumerate(m.parameters())})


if __name__ == '__main__':
    main()
