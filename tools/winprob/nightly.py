"""One scheduled run: crawl the newest patch for a while, refit when enough games are in, drop the
raw games of old patches.

python3 -I tools/winprob/nightly.py [--hours 2] [--games 20000] [--fit-at 10000] [--refit-every 2500] [--keep 2] [--dir DIR] [--key-file FILE] [--publish DIR]

Each patch has its own directory DIR/<patch> (raw/, crawl_state.json, rows.npz, fit/winprob.toml,
fit/winprob_cases.json, nightly.json); a crawl started by hand in DIR itself goes on there. The
fit runs once --fit-at games are stored and again every --refit-every more; fit.py writes its
files only when its gates pass, and nothing is ever written into the repo. --publish copies the
newest patch's fitted table to a directory and, when that is a git checkout, commits and pushes
it there (the app downloads it: src-tauri/src/winprob.rs). Patches older than the newest --keep
lose raw/ and rows.npz (0 keeps everything). No fit while League of Legends runs.
Everything is appended to DIR/nightly.log. One run at a time (DIR/nightly.lock). Exit status 1
when the crawl, the features, the fit or the push failed. README.md, "Every night", has the cron, systemd
and Task Scheduler lines.
"""
import argparse, json, os, re, shutil, subprocess, sys, time, tomllib
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import crawl  # noqa: E402
import features as F  # noqa: E402

HERE = Path(__file__).resolve().parent
PATCH_DIR = re.compile(r"\d+\.\d+")
STALE_LOCK_H = 26
GAME = "League of Legends.exe"


class Busy(Exception):
    pass


class Lock:
    """One run at a time in a data directory; a lock older than `STALE_LOCK_H` is a crashed run's."""

    def __init__(self, path):
        self.path = Path(path)

    def __enter__(self):
        if self.path.exists() and time.time() - self.path.stat().st_mtime > STALE_LOCK_H * 3600:
            self.path.unlink()
        try:
            os.close(os.open(self.path, os.O_CREAT | os.O_EXCL | os.O_WRONLY))
        except FileExistsError:
            raise Busy(f"{self.path} exists: another run is going on") from None
        return self

    def __exit__(self, *exc):
        self.path.unlink(missing_ok=True)


def state_of(d):
    p = Path(d) / "crawl_state.json"
    return json.loads(p.read_text(encoding="utf-8")) if p.exists() else {}


def stored(d):
    return len(state_of(d).get("stored", []))


def patch_dir(root, patch):
    return Path(root) if state_of(root).get("patch") == patch else Path(root) / patch


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
    """The crawl, the fit when one is due and the pruning; `sh(command)` gives an exit status."""
    d = patch_dir(a.dir, patch)
    d.mkdir(parents=True, exist_ok=True)
    py = lambda script, *args: [sys.executable, "-I", str(HERE / script), *map(str, args)]
    key = ["--key-file", a.key_file] if a.key_file else []
    failed = sh(py("crawl.py", "--games", a.games, "--hours", a.hours, "--patch", patch, "--platform", a.platform, "--region", a.region, "--out", d, *key)) != 0
    games = stored(d)
    say(f"patch {patch}: {games} games stored" + ("; the crawl failed (see above)" if failed else ""))
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


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--hours", type=float, default=2.0, help="how long to crawl")
    ap.add_argument("--games", type=int, default=20000, help="stop crawling a patch at this many games")
    ap.add_argument("--fit-at", type=int, default=10000)
    ap.add_argument("--refit-every", type=int, default=2500)
    ap.add_argument("--keep", type=int, default=2, help="patches that keep their raw games (0: all)")
    ap.add_argument("--platform", default="euw1")
    ap.add_argument("--region", default="europe")
    ap.add_argument("--dir", default=str(F.DEV))
    ap.add_argument("--key-file")
    ap.add_argument("--publish", help="directory (or git checkout) that gets the fitted winprob.toml")
    a = ap.parse_args()
    a.dir = Path(a.dir)
    a.dir.mkdir(parents=True, exist_ok=True)
    with open(a.dir / "nightly.log", "a", encoding="utf-8") as log:
        def say(text):
            line = f"{time.strftime('%Y-%m-%d %H:%M:%S')} {text}"
            print(line, flush=True)
            log.write(line + "\n")
            log.flush()

        def sh(cmd):
            return subprocess.run(cmd, stdout=log, stderr=subprocess.STDOUT).returncode

        try:
            with Lock(a.dir / "nightly.lock"):
                status = run(a, crawl.newest_patch(), sh, say)
        except Busy as e:
            say(str(e))
            status = 1
        except Exception as e:
            say(f"stopped: {type(e).__name__}: {e}")
            status = 1
    sys.exit(status)


if __name__ == "__main__":
    main()
