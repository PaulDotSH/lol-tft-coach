"""One scheduled run: crawl the newest patch for a while, refit when enough games are in, drop the
raw games of old patches. With --forever, one run after another.

python3 -I tools/winprob/nightly.py [--forever] [--servers euw1/europe kr/asia] [--hours 2] [--games 20000] [--fit-at 10000] [--refit-every 2500] [--keep 2] [--dir DIR] [--key-file FILE] [--publish DIR]

Each patch has its own directory DIR/<patch> (raw/, rows.npz, fit/winprob.toml,
fit/winprob_cases.json, nightly.json, and <platform>/crawl_state.json for each server); a crawl
started by hand in DIR itself, or in DIR/<patch>, goes on there. The servers (platform/region
of the Riot API) are crawled side by side, each to its share of --games, into the one raw/:
a key has one rate limit for each region, so two servers of one region are refused. The
fit runs once --fit-at games are stored in all and again every --refit-every more; fit.py writes its
files only when its gates pass, and nothing is ever written into the repo. --publish copies the
newest patch's fitted table to a directory and, when that is a git checkout, commits and pushes
it there (the app downloads it: src-tauri/src/winprob.rs). Patches older than the newest --keep
lose raw/ and rows.npz (0 keeps everything). No fit while League of Legends runs.
Everything is appended to DIR/nightly.log. One run at a time (DIR/nightly.lock). Exit status 1
when a crawl, the features, the fit or the push failed. --forever starts the next run when one
ends, `PAUSE_S` after the last start at the earliest, and never exits. README.md, "Every night"
and "Without a break", has the cron, systemd and Task Scheduler lines.
"""
import argparse, json, os, re, shutil, subprocess, sys, time, tomllib
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import crawl  # noqa: E402
import features as F  # noqa: E402

HERE = Path(__file__).resolve().parent
PATCH_DIR = re.compile(r"\d+\.\d+")
PAUSE_S = 600.0
GAME = "League of Legends.exe"


def state_of(d):
    p = Path(d) / "crawl_state.json"
    return json.loads(p.read_text(encoding="utf-8")) if p.exists() else {}


def stored(d):
    return len(state_of(d).get("stored", []))


def patch_dir(root, patch):
    return Path(root) if state_of(root).get("patch") == patch else Path(root) / patch


def server_dir(d, platform):
    return Path(d) if state_of(d).get("platform") == platform else Path(d) / platform


def servers(names):
    """[(platform, region)] from platform/region names; a region's rate limit takes one server."""
    pairs = [tuple(n.split("/")) for n in names]
    if not pairs or any(len(p) != 2 for p in pairs):
        sys.exit(f"servers are named platform/region, as in euw1/europe: {' '.join(names)!r}")
    regions = [r for _, r in pairs]
    if len(set(regions)) < len(regions):
        sys.exit(f"one server for each region, whose rate limit they would share: {' '.join(names)}")
    return pairs


def due(games, last, fit_at, every):
    """Whether to fit with `games` stored when the last fit had `last` (None: never fitted)."""
    return games >= (fit_at if last is None else last + every)


def prune(root, keep):
    """Removes raw/ and rows.npz of every patch directory but the newest `keep`; their names."""
    patches = sorted((p for p in Path(root).iterdir() if p.is_dir() and PATCH_DIR.fullmatch(p.name)), key=lambda p: F.version(p.name))
    gone = []
    for p in patches[:-keep] if keep > 0 else []:
        if (p / "raw").exists() or (p / "rows.npz").exists():
            shutil.rmtree(p / "raw", ignore_errors=True)
            (p / "rows.npz").unlink(missing_ok=True)
            gone.append(p.name)
    return gone


def publish(table, to, sh, say):
    """Copies a fitted table into `to`; when that is a git checkout, commits and pushes it.
    False when git failed."""
    to.mkdir(parents=True, exist_ok=True)
    dest = to / table.name
    if not dest.exists() or dest.read_bytes() != table.read_bytes():
        shutil.copyfile(table, dest)
        say(f"published {dest}")
    if not (to / ".git").exists():
        return True
    git = lambda *args: sh(["git", "-C", str(to), *args])
    model = tomllib.loads(table.read_text(encoding="utf-8"))
    done = (git("add", table.name) == 0
            and (git("diff", "--cached", "--quiet") == 0 or git("commit", "-q", "-m", f"winprob {model['patch']}: {model['games']} games") == 0)
            and git("push", "-q") == 0)
    if not done:
        say(f"git in {to} failed (see above)")
    return done


def playing():
    if os.name != "nt":
        return False
    try:
        return GAME in subprocess.run(["tasklist"], capture_output=True, text=True, timeout=30).stdout
    except (OSError, subprocess.SubprocessError):
        return False


def run(a, patch, sh, say, playing=playing):
    """The crawls, side by side, the fit when one is due and the pruning; `sh(command)` gives an
    exit status."""
    d = patch_dir(a.dir, patch)
    d.mkdir(parents=True, exist_ok=True)
    py = lambda script, *args: [sys.executable, "-I", str(HERE / script), *map(str, args)]
    key = ["--key-file", a.key_file] if a.key_file else []
    where = [(platform, region, server_dir(d, platform)) for platform, region in servers(a.servers)]
    share = -(-a.games // len(where))
    crawls = [py("crawl.py", "--games", share, "--hours", a.hours, "--patch", patch, "--platform", platform, "--region", region, "--out", out, "--raw", d / "raw", *key)
              for platform, region, out in where]
    with ThreadPoolExecutor(len(crawls)) as pool:
        failed = any(list(pool.map(sh, crawls)))
    games = sum(stored(out) for _, _, out in where)
    say(f"patch {patch}: {games} games stored" + (f" ({', '.join(f'{p} {stored(out)}' for p, _, out in where)})" if len(where) > 1 else "")
        + ("; a crawl failed (see above)" if failed else ""))
    memo_path = d / "nightly.json"
    memo = json.loads(memo_path.read_text(encoding="utf-8")) if memo_path.exists() else {}
    if due(games, memo.get("fit_games"), a.fit_at, a.refit_every):
        if playing():
            say("a fit is due, but League of Legends runs: next time")
        else:
            fit = d / "fit"
            fit.mkdir(exist_ok=True)
            passed = (sh(py("features.py", "--raw", d / "raw", "--out", d / "rows.npz", "--patch", patch)) == 0
                      and sh(py("fit.py", "--rows", d / "rows.npz", "--out", fit / "winprob.toml", "--cases", fit / "winprob_cases.json")) == 0)
            memo_path.write_text(json.dumps(dict(fit_games=games, passed=passed, at=time.strftime("%Y-%m-%d %H:%M"))), encoding="utf-8")
            say(f"fit on {games} games: " + (f"gates pass, table in {fit}" if passed else "failed or gates not passed (see above), nothing written"))
            failed = failed or not passed
    table = d / "fit" / "winprob.toml"
    if a.publish and table.exists():
        failed = not publish(table, Path(a.publish), sh, say) or failed
    for name in prune(a.dir, a.keep):
        say(f"removed the raw games of patch {name}")
    return int(failed)


def forever(once, pause=PAUSE_S, clock=time.monotonic, sleep=time.sleep):
    """`once()` again and again, a start at least `pause` seconds after the one before."""
    while True:
        started = clock()
        once()
        sleep(max(0.0, pause - (clock() - started)))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--forever", action="store_true", help="one run after another, for a service")
    ap.add_argument("--servers", nargs="+", default=["euw1/europe"], metavar="PLATFORM/REGION")
    ap.add_argument("--hours", type=float, default=2.0, help="how long to crawl in one run")
    ap.add_argument("--games", type=int, default=20000, help="stop crawling a patch at this many games, all servers together")
    ap.add_argument("--fit-at", type=int, default=10000)
    ap.add_argument("--refit-every", type=int, default=2500)
    ap.add_argument("--keep", type=int, default=2, help="patches that keep their raw games (0: all)")
    ap.add_argument("--dir", default=str(F.DEV))
    ap.add_argument("--key-file")
    ap.add_argument("--publish", help="directory (or git checkout) that gets the fitted winprob.toml")
    a = ap.parse_args()
    a.dir = Path(a.dir)
    a.dir.mkdir(parents=True, exist_ok=True)
    servers(a.servers)
    with open(a.dir / "nightly.log", "a", encoding="utf-8") as log:
        def say(text):
            line = f"{time.strftime('%Y-%m-%d %H:%M:%S')} {text}"
            print(line, flush=True)
            log.write(line + "\n")
            log.flush()

        def sh(cmd):
            return subprocess.run(cmd, stdout=log, stderr=subprocess.STDOUT).returncode

        def once():
            try:
                with crawl.Lock(a.dir / "nightly.lock"):
                    return run(a, crawl.newest_patch(), sh, say)
            except crawl.Busy as e:
                say(str(e))
            except Exception as e:
                say(f"stopped: {type(e).__name__}: {e}")
            return 1

        if a.forever:
            forever(once)
        status = once()
    sys.exit(status)


if __name__ == "__main__":
    main()
