"""Final boards of captured TFT games from the Riot API (ground truth for the eval set).

Finds my matches by puuid whose start lies within an hour before a capture dir's first frame
(frames are named by epoch ms) and prints every player's final units: name ★ items, placement.
Key: env RIOT_API_KEY (see docs/riot-api-key.md). Untested until a key is set.

py -3.12 -I tools/tft-render/riot_boards.py <capture dir>... [--name vandboabe --tag cox --route europe]
"""
import argparse, json, os, sys, time, urllib.error, urllib.parse, urllib.request


def get(route, path, key):
    req = urllib.request.Request(f'https://{route}.api.riotgames.com{path}', headers={'X-Riot-Token': key})
    for _ in range(5):
        try:
            return json.load(urllib.request.urlopen(req, timeout=30))
        except urllib.error.HTTPError as e:
            if e.code != 429:
                raise
            time.sleep(int(e.headers.get('Retry-After', '10')))
    raise RuntimeError('rate limited')


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('dirs', nargs='+')
    ap.add_argument('--name', default='vandboabe')
    ap.add_argument('--tag', default='cox')
    ap.add_argument('--route', default='europe')
    a = ap.parse_args()
    key = os.environ.get('RIOT_API_KEY') or sys.exit('RIOT_API_KEY not set')
    puuid = get(a.route, f'/riot/account/v1/accounts/by-riot-id/{urllib.parse.quote(a.name)}/{urllib.parse.quote(a.tag)}', key)['puuid']
    ids = get(a.route, f'/tft/match/v1/matches/by-puuid/{puuid}/ids?count=30', key)
    matches = [get(a.route, f'/tft/match/v1/matches/{i}', key) for i in ids]
    for d in a.dirs:
        first = min(int(f[:-4]) for f in os.listdir(d) if f.endswith('.png'))
        # game_datetime is the match end (ms); the game started before the first frame.
        m = next((m for m in matches if first - 3600_000 <= m['info']['game_datetime'] - m['info']['game_length'] * 1000 <= first + 300_000), None)
        print(f'== {d}: {m["metadata"]["match_id"] if m else "no match found"}')
        if not m:
            continue
        for p in sorted(m['info']['participants'], key=lambda p: p['placement']):
            units = ' '.join(f"{u['character_id']}*{u['tier']}" + (f"[{','.join(u.get('itemNames', []))}]" if u.get('itemNames') else '') for u in p['units'])
            who = p.get('riotIdGameName', p['puuid'][:8])
            print(f"  #{p['placement']} {who} lvl {p['level']} round {p['last_round']}: {units}")


if __name__ == '__main__':
    main()
