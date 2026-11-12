"""Crawls ranked games (match + timeline) from Riot match-v5 for the win-probability fit.

py -3.12 -I tools/winprob/crawl.py --games 10000 [--hours 2] [--platform euw1 --region europe] [--queue 420] [--patch 16.22] [--days 14] [--out DIR] [--raw DIR] [--key-file FILE] [--no-apex]

Key: --key-file, else env RIOT_API_KEY, else riot-api-key.txt at the repo root (docs/riot-api-key.md);
never printed. Ladder players, Emerald and above, the highest first (Challenger, Grandmaster and
Master, which alone fill a crawl of 20,000 games, then Diamond and Emerald divisions page by page,
round robin; --no-apex starts at Diamond), their recent match ids, newest first, each match and
its timeline when the match is on the patch; a match of an older patch ends that player's list.
Raw JSON is gzipped in DIR/raw (default `features.DEV`; --raw names another directory, which
crawls of other servers can share: a match id starts with its server). Resumable: the queue of players, the ladder
pages read and every match id seen are kept in DIR/crawl_state.json; player ids only work with the
key that read them, so after a change of key the ladder is read again. Stops at --games on disk or
after --hours. One crawl at a time in DIR (DIR/crawl.lock). One limiter for 20 requests / s and 100 / 2 min; a 429 waits for its Retry-After.
"""
import argparse, collections, gzip, json, os, sys, time
from pathlib import Path

import requests

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import features as F  # noqa: E402
import riot_format as R  # noqa: E402

LIMITS = ((20, 1.0), (100, 120.0))
LIMIT_PAD = 0.1
RETRIES = 8
RETRY_AFTER = 10
PROGRESS = 100
KEY_ENV = "RIOT_API_KEY"
KEY_FILE = Path(__file__).resolve().parents[2] / "riot-api-key.txt"


class KeyRejected(Exception):
    pass


class Busy(Exception):
    pass


class Lock:
    """One process at a time: an OS lock on the file, which goes with the process that holds it."""

    def __init__(self, path):
        self.path = Path(path)

    def __enter__(self):
        self.f = open(self.path, "a+")
        try:
            if os.name == "nt":
                import msvcrt
                self.f.seek(0)
                msvcrt.locking(self.f.fileno(), msvcrt.LK_NBLCK, 1)
            else:
                import fcntl
                fcntl.flock(self.f, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError:
            self.f.close()
            raise Busy(f"{self.path} is held: another run is going on") from None
        return self

    def __exit__(self, *exc):
        self.f.close()


class OtherKey(Exception):
    """A player id that was read with another key: Riot encrypts puuids for each key."""


class Limiter:
    """Sleeps before a request until every (count, seconds) window has room."""

    def __init__(self, limits=LIMITS, clock=time.monotonic, sleep=time.sleep):
        self.limits, self.times, self.clock, self.sleep = limits, collections.deque(), clock, sleep

    def wait(self):
        while True:
            now = self.clock()
            delay = 0.0
            for n, span in self.limits:
                recent = [t for t in self.times if now - t < span]
                if len(recent) >= n:
                    delay = max(delay, span - (now - recent[-n]) + LIMIT_PAD)
            if delay <= 0:
                break
            self.sleep(delay)
        self.times.append(self.clock())
        while self.times and self.clock() - self.times[0] > max(s for _, s in self.limits):
            self.times.popleft()


class Api:
    def __init__(self, key, platform, region):
        self.s = requests.Session()
        self.s.headers[R.KEY_HEADER] = key
        self.s.headers["User-Agent"] = "lol-coach winprob crawl"
        self.platform = R.PLATFORM_HOST.format(platform=platform)
        self.region = R.REGION_HOST.format(region=region)
        self.limiter, self.requests = Limiter(), 0

    def get(self, host, path, params=None):
        """JSON reply, or None on 404."""
        for attempt in range(RETRIES):
            self.limiter.wait()
            self.requests += 1
            r = self.s.get(host + path, params=params, timeout=30)
            if r.status_code == 429:
                time.sleep(int(r.headers.get("Retry-After", RETRY_AFTER)))
                continue
            if r.status_code >= 500:
                time.sleep(min(60, 2 ** attempt))
                continue
            if r.status_code == 404:
                return None
            if r.status_code in (401, 403):
                raise KeyRejected(f"{r.status_code} on {path}: the key is missing, mistyped or expired (dev keys last 24 h)")
            if r.status_code == 400 and R.OTHER_KEY in r.text:
                raise OtherKey(f"400 on {path}: a player id that was read with another key")
            r.raise_for_status()
            return r.json()
        raise RuntimeError(f"gave up on {path} after {RETRIES} tries")


def read_key(path=None, env=os.environ, default=KEY_FILE):
    """The key: the first word of the file `path` when one is named, else env RIOT_API_KEY, else `default`."""
    from_file = lambda p: (p.read_text(encoding="utf-8-sig").split() or [""])[0] if p.exists() else ""
    key = from_file(Path(path)) if path else env.get(KEY_ENV, "").strip() or from_file(Path(default))
    return key or sys.exit(f"no Riot key: {path or f'{KEY_ENV} is not set and {default} is missing'} (docs/riot-api-key.md)")


def newest_patch():
    return ".".join(requests.get(R.DDRAGON_VERSIONS, timeout=30).json()[0].split(".")[:2])


def ladder_order():
    return [f"{t} {d}" for d in R.LADDER_DIVISIONS for t in R.LADDER_TIERS]


def load_state(path, a):
    if path.exists():
        s = json.loads(path.read_text(encoding="utf-8"))
        for k in ("platform", "region", "queue", "patch"):
            if s[k] != getattr(a, k):
                sys.exit(f"{path} was started with {k} {s[k]!r}, not {getattr(a, k)!r}: use another --out")
        return s
    return dict(platform=a.platform, region=a.region, queue=a.queue, patch=a.patch, **no_players(), seen=[], stored=[])


def no_players():
    return dict(apex_done=[], pages={d: 1 for d in ladder_order()}, exhausted=[], turn=0, puuids=[], next_puuid=0)


def save_state(path, s):
    tmp = path.with_suffix(".tmp")
    tmp.write_text(json.dumps(s), encoding="utf-8")
    os.replace(tmp, path)


def more_players(api, s, apex=True):
    """Adds one apex league (unless `apex` is off) or one ladder page of puuids; False when the ladder is used up."""
    known = set(s["puuids"])
    add = lambda entries: s["puuids"].extend(p for p in (R.need(e, R.ENTRY_PUUID, "league entry") for e in entries) if p not in known)
    for tier, path in R.LEAGUE_APEX.items() if apex else ():
        if tier not in s["apex_done"]:
            league = api.get(api.platform, path.format(queue=R.LEAGUE_QUEUE)) or {}
            add(R.need(league, R.APEX_ENTRIES, f"{tier} league") if league else [])
            s["apex_done"].append(tier)
            return True
    order = [d for d in ladder_order() if d not in s["exhausted"]]
    if not order:
        return False
    div = order[s["turn"] % len(order)]
    s["turn"] += 1
    tier, division = div.split()
    entries = api.get(api.platform, R.LEAGUE_ENTRIES.format(queue=R.LEAGUE_QUEUE, tier=tier, division=division), {"page": s["pages"][div]}) or []
    s["pages"][div] += 1
    if not entries:
        s["exhausted"].append(div)
    add(entries)
    return True


def save_gz(path, obj):
    tmp = path.with_suffix(".tmp")
    with gzip.open(tmp, "wt", encoding="utf-8") as f:
        json.dump(obj, f)
    os.replace(tmp, path)


def crawl(api, s, save, raw, games, patch, queue, since, until=None, clock=time.monotonic, apex=True):
    """Stores games until `games` are on disk, `clock()` reaches `until` or the ladder is used up;
    `save()` keeps `s`. Players read with another key are dropped once and the ladder read again."""
    seen = set(s["seen"])
    started, reread = clock(), False
    over = lambda: until is not None and clock() >= until

    def done(mid):
        seen.add(mid)
        s["seen"].append(mid)

    while len(s["stored"]) < games and not over():
        if s["next_puuid"] >= len(s["puuids"]):
            if not more_players(api, s, apex):
                print("ladder used up")
                break
            save()
            continue
        puuid = s["puuids"][s["next_puuid"]]
        try:
            ids = api.get(api.region, R.MATCH_IDS.format(puuid=puuid), {"queue": queue, "startTime": since, "start": 0, "count": R.MATCH_IDS_PAGE}) or []
        except OtherKey:
            if reread:
                raise
            reread = True
            print(f"the {len(s['puuids'])} players were read with another key: reading the ladder again", flush=True)
            s.update(no_players())
            save()
            continue
        stop = False
        for mid in ids:
            if mid in seen:
                continue
            stop = len(s["stored"]) >= games or over()
            if stop:
                break
            match = api.get(api.region, R.MATCH.format(match_id=mid))
            if match is None or F.patch_of(match) != patch:
                done(mid)
                if match is not None and F.version(F.patch_of(match)) < F.version(patch):
                    break
                continue
            timeline = api.get(api.region, R.TIMELINE.format(match_id=mid))
            if timeline is not None:
                save_gz(raw / f"{mid}.match.json.gz", match)
                save_gz(raw / f"{mid}.timeline.json.gz", timeline)
                s["stored"].append(mid)
            done(mid)
            save()
            if timeline is not None and len(s["stored"]) % PROGRESS == 0:
                h = (clock() - started) / 3600
                print(f"{s['platform']}: {len(s['stored'])} games, {api.requests} requests, player {s['next_puuid']} of {len(s['puuids'])}, {h:.2f} h", flush=True)
        if not stop:
            s["next_puuid"] += 1
        save()


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--games", type=int, required=True)
    ap.add_argument("--hours", type=float, help="stop after this long (default: no limit)")
    ap.add_argument("--platform", default="euw1")
    ap.add_argument("--region", default="europe")
    ap.add_argument("--queue", type=int, default=420)
    ap.add_argument("--patch", help="major.minor (default: the newest on Data Dragon)")
    ap.add_argument("--days", type=int, default=14, help="match ids from this many days back")
    ap.add_argument("--out", default=str(F.DEV))
    ap.add_argument("--raw", help="where the games go (default: OUT/raw)")
    ap.add_argument("--key-file")
    ap.add_argument("--no-apex", dest="apex", action="store_false", help="leave out Challenger, Grandmaster and Master players")
    a = ap.parse_args()
    key = read_key(a.key_file)
    a.patch = a.patch or newest_patch()
    out = Path(a.out)
    raw = Path(a.raw) if a.raw else out / "raw"
    raw.mkdir(parents=True, exist_ok=True)
    out.mkdir(parents=True, exist_ok=True)
    state_path = out / "crawl_state.json"
    try:
        with Lock(out / "crawl.lock"):
            s = load_state(state_path, a)
            save = lambda: save_state(state_path, s)
            print(f"patch {a.patch}, queue {a.queue}, {a.platform}/{a.region}: {len(s['stored'])} games stored, target {a.games}", flush=True)
            try:
                crawl(Api(key, a.platform, a.region), s, save, raw, a.games, a.patch, a.queue, int(time.time()) - a.days * 86400,
                      time.monotonic() + a.hours * 3600 if a.hours else None, apex=a.apex)
            except (KeyRejected, OtherKey) as e:
                save()
                sys.exit(str(e))
            except KeyboardInterrupt:
                save()
                sys.exit("stopped; run again to resume")
            save()
            print(f"stopped: {a.platform} has {len(s['stored'])} games in {raw}")
    except Busy as e:
        sys.exit(str(e))


if __name__ == "__main__":
    main()
