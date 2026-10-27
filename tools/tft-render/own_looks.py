"""My own units' real looks for training (approach 2 with real data): the app's live library
(`%LOCALAPPDATA%\\lol-coach\\tft\\looks.bin`) and the replays' confirmed pieces (eval `mine` crops),
each kept only if the unit was on my final board (client TFT history): of that game for replay
crops, of any recent game for the live library (it has no game ids). Writes `own.npz` (embeddings,
apis). Usage: py -3.12 -I tools/tft-render/own_looks.py <history json> <out.npz>
"""
import json
import os
import struct
import sys
from collections import Counter

import numpy as np

sys.path.insert(0, os.path.dirname(__file__))
import autolabel as A  # noqa: E402
import calibrate as C  # noqa: E402

LIVE = os.path.expandvars(r'%LOCALAPPDATA%\lol-coach\tft\looks.bin')


def read_library(path):
    b = open(path, 'rb').read()
    out, i = [], 4
    f16 = b[:4] == b'TFL2'
    while i < len(b):
        n = struct.unpack_from('<H', b, i)[0]
        i += 2
        api = b[i:i + n].decode()
        i += n + 1 + (1 if f16 else 0)
        d = struct.unpack_from('<H', b, i)[0]
        i += 2
        size = 2 if f16 else 4
        out.append((api, np.frombuffer(b[i:i + size * d], '<f2' if f16 else '<f4').astype(np.float32)))
        i += size * d
    return out


def my_finals(history):
    """gameCreation (s) → my final units."""
    d = json.load(open(history, encoding='utf8'))
    out = {}
    for g in d['games']:
        j = g['json']
        me = [p for p in j['participants'] if p.get('riotIdGameName', '').lower() == A.ME]
        if me:
            out[j['gameCreation'] / 1000] = {u['character_id'] for u in me[0]['units']}
    return out


def main():
    history, out = sys.argv[1], sys.argv[2]
    finals = my_finals(history)
    recent = set().union(*[u for t, u in sorted(finals.items())[-8:]])
    kept, dropped = [], Counter()
    for api, e in read_library(LIVE) if os.path.exists(LIVE) else []:
        (kept.append((api, e, 'live')) if api in recent else dropped.update([api]))
    from datetime import datetime
    for cap in sorted(os.listdir(C.EVAL)):
        idx = os.path.join(C.EVAL, cap, 'crops.jsonl')
        if not os.path.exists(idx):
            continue
        t = datetime.strptime(cap, '%Y%m%d-%H%M%S').timestamp()
        start = min(finals, key=lambda s: abs(s - t))
        final = finals[start] if abs(start - t) < 600 else set()
        for l in open(idx, encoding='utf8'):
            j = json.loads(l)
            if j['group'] == 'mine' and j.get('api'):
                e = np.fromfile(os.path.join(C.EVAL, cap, j['file'][:-4] + '.f32'), '<f4')
                (kept.append((j['api'], e, cap)) if j['api'] in final else dropped.update([j['api']]))
    print(f'kept {len(kept)} looks of {len({k[0] for k in kept})} units: {Counter(k[0] for k in kept).most_common()}')
    print(f'dropped (not on my final board): {sum(dropped.values())} {dropped.most_common()}')
    np.savez(out, embs=np.stack([k[1] for k in kept]), apis=np.array([k[0] for k in kept]), src=np.array([k[2] for k in kept]))


if __name__ == '__main__':
    main()
