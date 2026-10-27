"""Label combat-start enemy crops from the League client's TFT match history (every player's final
board), without a Riot API key. Per fight: the opponent is the player whose final board explains
the fight's crops best (a classifier's probabilities restricted to that board's units), among
players still alive and in their last LATE rounds; a read name plate (`who`) decides outright.
Each crop then gets its board's most likely unit when it leads clearly.
Checks the labels against the hand labels. Usage:
py -3.12 -I tools/tft-render/autolabel.py <history json> <dump dir>... [--out labels_auto.jsonl]
"""
import json
import os
import sys
from collections import defaultdict
from datetime import datetime

import numpy as np
import torch

sys.path.insert(0, os.path.dirname(__file__))
import calibrate as C  # noqa: E402
import head as H  # noqa: E402

ME = 'vandboabe'
# A player's board counts as their final board this many rounds before their last round.
LATE = 8
# The chosen board must explain the fight this much better (mean log-probability) than the next.
BOARD_LEAD = 0.5
# A crop's unit must have this share of its board's probability.
UNIT_SHARE = 0.6


def round_index(stage, r):
    return r if stage == 1 else 4 + (stage - 2) * 7 + r


def games(history):
    """Capture dir → that game's participants (name, final units, last round)."""
    d = json.load(open(history, encoding='utf8'))
    by_start = {}
    for g in d['games']:
        j = g['json']
        by_start[j['gameCreation'] / 1000] = j
    out = {}
    for cap in sorted(os.listdir(C.EVAL)):
        try:
            t = datetime.strptime(cap, '%Y%m%d-%H%M%S').timestamp()
        except ValueError:
            continue
        start = min(by_start, key=lambda s: abs(s - t))
        if abs(start - t) > 600:
            continue
        j = by_start[start]
        out[cap] = [{'name': p.get('riotIdGameName', ''), 'units': {u['character_id'] for u in p['units']}, 'last': p['last_round']}
                    for p in j['participants'] if p.get('riotIdGameName', '').lower() != ME]
    return out


def fights(cap):
    """(stage, frame, who) → [(x, y, embedding)] of the fight's first full read."""
    out = defaultdict(list)
    for l in open(os.path.join(C.EVAL, cap, 'crops.jsonl'), encoding='utf8'):
        j = json.loads(l)
        if j['group'] == 'combat' and j.get('stage'):
            key = (tuple(j['stage']), j['frame'], j.get('who') or '')
            out[key].append((j['x'], j['y'], np.fromfile(os.path.join(C.EVAL, cap, j['file'][:-4] + '.f32'), '<f4')))
    return out


def main():
    out_path = sys.argv[sys.argv.index('--out') + 1] if '--out' in sys.argv else None
    args = [a for a in sys.argv[1:] if not a.startswith('--') and a != out_path]
    history, dirs = args[0], args[1:]
    parts = [C.load_dump(d) for d in dirs]
    X = np.concatenate([p[0] for p in parts])
    meta = sum([p[1] for p in parts], [])
    units = sorted({m['api'] for m in meta})
    y = np.array([units.index(m['api']) for m in meta])
    model = H.train(X, y, len(units), 512, epochs=15)
    hand = {}
    for l in open(C.LABELS, encoding='utf8'):
        j = json.loads(l)
        hand[(j['game'], j['frame'], round(j['x']), round(j['y']))] = j['api']
    labels, agree, disagree, skipped = [], 0, 0, 0
    for cap, players in games(history).items():
        for (stage, frame, who), crops in sorted(fights(cap).items()):
            rnd = round_index(*stage)
            cands = [p for p in players if p['last'] - LATE <= rnd <= p['last']]
            named = [p for p in cands if who and who.lower() == p['name'].lower()]
            with torch.no_grad():
                logp = torch.log_softmax(model(torch.tensor(np.stack([c[2] for c in crops]))), -1).numpy()
            def fit(p):
                idx = [units.index(u) for u in p['units'] if u in units]
                return float(np.mean(logp[:, idx].max(1))) if idx else -1e9
            scored = sorted(((fit(p), p) for p in cands), key=lambda s: -s[0])
            if named:
                board = named[0]
            elif len(scored) >= 2 and scored[0][0] - scored[1][0] >= BOARD_LEAD:
                board = scored[0][1]
            elif len(scored) == 1:
                board = scored[0][1]
            else:
                skipped += len(crops)
                continue
            idx = [units.index(u) for u in board['units'] if u in units]
            for (x, yy, _), lp in zip(crops, logp):
                p = np.exp(lp[idx])
                p = p / p.sum()
                k = int(np.argmax(p))
                if p[k] < UNIT_SHARE:
                    skipped += 1
                    continue
                api = units[idx[k]]
                labels.append({'game': cap, 'frame': frame, 'x': x, 'y': yy, 'group': 'combat', 'api': api, 'player': board['name'], 'share': round(float(p[k]), 3)})
                h = hand.get((cap, frame, round(x), round(yy)))
                if h and h not in ('?',):
                    agree += h == api
                    disagree += h != api
    print(f'auto labels: {len(labels)}, skipped crops: {skipped}; against hand labels: {agree} agree, {disagree} disagree')
    if out_path:
        with open(out_path, 'w', encoding='utf8', newline='\n') as f:
            f.writelines(json.dumps(l) + '\n' for l in labels)


if __name__ == '__main__':
    main()
