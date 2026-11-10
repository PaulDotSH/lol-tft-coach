"""Label real unit crops from the League client's TFT match history (every player's final board),
second version (spec 2026-11-10, track B). Per fight (a game folder's `combat` crops of one read):

1. Link it to a player (track A): the app's `fights.jsonl` (plate-linked opponent, HP-change
   candidates) when there is one, else the read's plate (`who`); candidates are the players alive at
   that round (match history `last_round`). One candidate links; several are decided by the best
   explanation (the board whose units explain the crops best, BOARD_MARGIN ahead of the next).
2. Keep it only when the round is within LABEL_ROUNDS of the player's last round.
3. Score every crop against every unit of that final board (a head's probabilities renormalised
   over the board) and solve the assignment (Hungarian) with a "none" option per crop; a star read
   from the bar allows only units at least that star on the final board (a 3-star crop takes a 3-star unit).
4. Keep an assigned label only when it beats the crop's next choice by LABEL_MARGIN (log).
`mine` crops are labelled by the app's own confirmation, without a margin.

Reports the link (fights linked, how, against the hand labels: a fight's truth is the alive player
whose final board holds most of its hand-labelled units, at least half and alone) and the labels
(kept, agreeing with the hand labels). Usage:
py -3.12 -I tools/tft-render/autolabel.py <history json> <dump dir>... [--eval DIR] [--out labels_auto.jsonl] [--json report.json]
"""
import json
import os
import sys
from collections import defaultdict
from datetime import datetime

import numpy as np
from scipy.optimize import linear_sum_assignment

sys.path.insert(0, os.path.dirname(__file__))

ME = 'vandboabe'
LABEL_ROUNDS = 4
LABEL_MARGIN = 1.0
BOARD_MARGIN = 0.5
NONE_LOGP = np.log(0.15)
BLOCK = 1e6
GAME_MATCH = 600


def round_index(stage, r):
    return r if stage == 1 else 4 + (stage - 2) * 7 + r


def game_start(name):
    """A game folder's start (unix seconds): `YYYYMMDD-HHMMSS` (replays, local time) or the app's unix seconds."""
    if name.isdigit():
        return int(name)
    try:
        return datetime.strptime(name, '%Y%m%d-%H%M%S').timestamp()
    except ValueError:
        return None


def players_of(history, start, me=ME):
    """The other players of the history game nearest `start` (an hour either way too: local-time folder names
    repeat across a DST change): name, final units [(api, star)], last round."""
    d = json.load(open(history, encoding='utf8'))
    games = [g['json'] for g in d['games']]
    if not games or start is None:
        return None
    off = lambda g: min(abs(g['gameCreation'] / 1000 - start - dst) for dst in (0, 3600, -3600))
    j = min(games, key=off)
    if off(j) > GAME_MATCH:
        return None
    return [{'name': p.get('riotIdGameName', ''), 'units': [(u['character_id'], u.get('tier', 1)) for u in p['units']], 'last': p['last_round']}
            for p in j['participants'] if p.get('riotIdGameName', '').lower() != me]


def same(a, b):
    a, b = ''.join(c for c in (a or '').lower() if c.isalnum()), ''.join(c for c in (b or '').lower() if c.isalnum())
    return bool(a) and (a == b or (len(a) >= 6 and len(a) == len(b) and sum(x != y for x, y in zip(a, b)) <= 1))


def find(players, name):
    return next((p for p in players if same(p['name'], name)), None)


def fights_of(game_dir):
    """[(stage, frame, who, [crop record])] of the combat crops, and the app's fights by stage."""
    by = defaultdict(list)
    for l in open(os.path.join(game_dir, 'crops.jsonl'), encoding='utf8'):
        j = json.loads(l)
        if j['group'] == 'combat' and j.get('stage'):
            by[(tuple(j['stage']), j['frame'])].append(j)
    out = [(s, f, next((c.get('player') or c.get('who') for c in cs if c.get('player') or c.get('who')), None), cs) for (s, f), cs in sorted(by.items())]
    app = {}
    path = os.path.join(game_dir, 'fights.jsonl')
    if os.path.exists(path):
        for l in open(path, encoding='utf8'):
            j = json.loads(l)
            app[tuple(j['round'])] = j
    return out, app


def board_fit(logp, units, board):
    """Mean over crops of the best log-probability among `board`'s units."""
    idx = sorted({units.index(a) for a, _ in board if a in units})
    return float(np.mean(logp[:, idx].max(1))) if idx else -1e9


def link(stage, who, app, players, logp, units):
    """(player, how): how is 'plate', 'hp' (one candidate left), 'fit' (best explanation), or None unlinked."""
    rnd = round_index(*stage)
    alive = [p for p in players if p['last'] >= rnd]
    rec = app.get(tuple(stage))
    if rec and rec.get('opponent'):
        p = find(alive, rec['opponent'])
        if p:
            return p, 'plate'
    if who:
        p = find(alive, who)
        if p:
            return p, 'plate'
    cands = alive
    if rec and rec.get('candidates'):
        cands = [p for p in alive if any(same(p['name'], c) for c in rec['candidates'])] or alive
    if len(cands) == 1:
        return cands[0], 'hp'
    if logp is None:
        return None, None
    scored = sorted(((board_fit(logp, units, p['units']), i) for i, p in enumerate(cands)), reverse=True)
    if len(scored) >= 2 and scored[0][0] - scored[1][0] >= BOARD_MARGIN:
        return cands[scored[0][1]], 'fit'
    return None, None


def assign(logp, stars, board, units):
    """Labels per crop (api or None): Hungarian over the board's slots plus a "none" per crop, star-constrained,
    kept only LABEL_MARGIN ahead of the crop's next allowed choice. `logp`: crops x units (log-probabilities)."""
    slots = [(a, s) for a, s in board if a in units]
    if not slots or len(logp) == 0:
        return [None] * len(logp)
    idx = sorted({units.index(a) for a, _ in slots})
    sub = logp[:, idx]
    sub = sub - np.log(np.exp(sub).sum(1, keepdims=True))
    col = {units[k]: i for i, k in enumerate(idx)}
    n, m = len(logp), len(slots)
    ok = lambda c, s: stars[c] <= 0 or s >= stars[c]
    cost = np.full((n, m + n), BLOCK)
    for c in range(n):
        for j, (a, s) in enumerate(slots):
            if ok(c, s):
                cost[c, j] = -sub[c, col[a]]
        cost[c, m + c] = -NONE_LOGP
    rows, cols = linear_sum_assignment(cost)
    out = [None] * n
    for c, j in zip(rows, cols):
        if j >= m or cost[c, j] >= BLOCK:
            continue
        a = slots[j][0]
        others = [sub[c, col[b]] for b, s in slots if b != a and ok(c, s)] + [NONE_LOGP]
        if sub[c, col[a]] - max(others) >= LABEL_MARGIN:
            out[c] = a
    return out


def label_fight(stage, crops, player, logp, units):
    """Labels of one linked fight's crops (none outside LABEL_ROUNDS of the player's last round)."""
    rnd = round_index(*stage)
    if player is None or not (player['last'] - LABEL_ROUNDS <= rnd <= player['last']):
        return [None] * len(crops)
    return assign(logp, [c.get('star') or 0 for c in crops], player['units'], units)


def truth(stage, hand, players):
    """The alive player whose final board holds most of the fight's hand-labelled units (at least half, alone)."""
    if not hand:
        return None
    rnd = round_index(*stage)
    cover = sorted(((sum(any(a == u for u, _ in p['units']) for a in hand), i) for i, p in enumerate(players) if p['last'] >= rnd), reverse=True)
    if not cover or cover[0][0] * 2 < len(hand) or (len(cover) > 1 and cover[1][0] == cover[0][0]):
        return None
    return players[cover[0][1]]


def hand_labels(path):
    hand = {}
    for l in open(path, encoding='utf8'):
        j = json.loads(l)
        hand[(j['game'], j['frame'], round(j['x']), round(j['y']))] = j['api']
    return hand


def main():
    import torch
    import calibrate as C
    import head as H
    opt = lambda k: sys.argv[sys.argv.index(k) + 1] if k in sys.argv else None
    out_path, report_path, eval_dir = opt('--out'), opt('--json'), opt('--eval') or C.EVAL
    args = [a for a in sys.argv[1:] if not a.startswith('--') and a not in (out_path, report_path, eval_dir)]
    history, dirs = args[0], args[1:]
    parts = [C.load_dump(d) for d in dirs]
    X = np.concatenate([p[0] for p in parts])
    meta = sum([p[1] for p in parts], [])
    units = sorted({m['api'] for m in meta})
    y = np.array([units.index(m['api']) for m in meta])
    model = H.train(X, y, len(units), 512, epochs=15)
    hand = hand_labels(C.LABELS)
    labels, rep = [], defaultdict(int)
    per_game = {}
    for game in sorted(os.listdir(eval_dir)):
        gdir = os.path.join(eval_dir, game)
        if not os.path.exists(os.path.join(gdir, 'crops.jsonl')):
            continue
        players = players_of(history, game_start(game))
        if not players:
            print(f'{game}: no history game')
            continue
        fights, app = fights_of(gdir)
        g = defaultdict(int)
        for stage, frame, who, crops in fights:
            embs = np.stack([np.fromfile(os.path.join(gdir, c['file'][:-4] + '.f32'), '<f4') for c in crops])
            with torch.no_grad():
                logp = torch.log_softmax(model(torch.tensor(embs)), -1).numpy()
            rnd = round_index(*stage)
            g['fights'] += 1
            g['alive'] += sum(p['last'] >= rnd for p in players)
            g['plate_read'] += bool(who)
            p, how = link(stage, who, app, players, logp, units)
            hs = [hand.get((game, frame, round(c['x']), round(c['y']))) for c in crops]
            t = truth(stage, [h for h in hs if h and h not in ('?', '-')], players)
            g['checkable'] += t is not None
            if p is not None:
                g['linked'] += 1
                g[f'linked_{how}'] += 1
                if t is not None:
                    g['right' if t is p else 'wrong'] += 1
            for c, h, a in zip(crops, hs, label_fight(stage, crops, p, logp, units)):
                g['crops'] += 1
                if a is None:
                    continue
                labels.append({'game': game, 'frame': frame, 'x': c['x'], 'y': c['y'], 'group': 'combat', 'api': a, 'player': p['name'], 'how': how})
                g['labels'] += 1
                if h and h != '?':
                    g['label_agree' if h == a else 'label_disagree'] += 1
        per_game[game] = dict(g)
        for k, v in g.items():
            rep[k] += v
        for l in open(os.path.join(gdir, 'crops.jsonl'), encoding='utf8'):
            j = json.loads(l)
            if j['group'] == 'mine' and j.get('api'):
                labels.append({'game': game, 'frame': j['frame'], 'x': j['x'], 'y': j['y'], 'group': 'mine', 'api': j['api'], 'player': ME, 'how': 'mine'})
    for game, g in per_game.items():
        print(game, g)
    r = dict(rep)
    print(f"fights {r.get('fights', 0)}: linked {r.get('linked', 0)} (plate {r.get('linked_plate', 0)}, one candidate {r.get('linked_hp', 0)}, best fit {r.get('linked_fit', 0)}); "
          f"mean alive others {r.get('alive', 0) / max(1, r.get('fights', 0)):.1f}; checkable {r.get('checkable', 0)}: right {r.get('right', 0)}, wrong {r.get('wrong', 0)}; "
          f"combat labels {r.get('labels', 0)} of {r.get('crops', 0)} crops: agree {r.get('label_agree', 0)}, disagree {r.get('label_disagree', 0)}")
    if out_path:
        with open(out_path, 'w', encoding='utf8', newline='\n') as f:
            f.writelines(json.dumps(l) + '\n' for l in labels)
    if report_path:
        json.dump({'total': r, 'games': per_game}, open(report_path, 'w', encoding='utf8'), indent=1)


if __name__ == '__main__':
    main()
