"""Approach 2 with real data: the head trained on the composites plus my own units' real looks
(own_looks.py, weighted REAL_WEIGHT), gated on the combat crops (none of them are own looks).
Usage: py -3.12 -I tools/tft-render/head_own.py <own.npz> <dump dir>...
"""
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(__file__))
import calibrate as C  # noqa: E402
import gate as G  # noqa: E402
import head as H  # noqa: E402


def main():
    own = np.load(sys.argv[1])
    parts = [C.load_dump(d) for d in sys.argv[2:]]
    X = np.concatenate([p[0] for p in parts])
    meta = sum([p[1] for p in parts], [])
    units = sorted({m['api'] for m in meta})
    y = np.array([units.index(m['api']) for m in meta])
    keep = np.array([a in units for a in own['apis']])
    Xo, yo = own['embs'][keep], np.array([units.index(a) for a in own['apis'][keep]])
    combat = [c for c in G.crops() if c[0] == 'combat' and (c[1] == '-' or c[1] in units)]
    print(f'{len(units)} units, {len(X)} composites, {len(Xo)} own looks of {len(set(yo))} units, {len(combat)} combat crops')
    for w in (0, 20, 60, 200):
        Xt = np.concatenate([X, np.repeat(Xo, w, 0)]) if w else X
        yt = np.concatenate([y, np.repeat(yo, w)]) if w else y
        for hidden in (0, 512):
            m = H.train(Xt, yt, len(units), hidden, epochs=15)
            n, t1, t3, b = H.gate(m, combat, units)['combat']
            print(f'own weight={w:<4} hidden={hidden:<4} combat n={n} top1={t1:.2f} top3={t3:.2f} named right={b[0]:.2f} wrong={b[2]:.2f} at {b[1]}')


if __name__ == '__main__':
    main()
