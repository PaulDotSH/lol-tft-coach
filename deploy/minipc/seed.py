"""Turns a crawl made on a PC by tools/winprob/crawl.py into what the service reads, so the minipc
goes on from it instead of starting at nothing. Same Riot key on both, or the player ids do not work.

py -3.12 -I deploy/minipc/seed.py PC_CRAWL_DIR OUT_DIR

PC_CRAWL_DIR holds crawl_state.json and raw/; OUT_DIR gets winprob/<patch>/<platform>.json (the
service's state) and a list of the raw files to send (winprob/<patch>/raw.txt). Send both to
~/lol-coach/import on the minipc (README.md here); install.sh moves them in. Stop the PC crawl
first. Only for a server the service has not crawled yet on that patch.
"""
import json, sys
from pathlib import Path

LADDER = ("apex_done", "pages", "exhausted", "turn", "puuids", "next_puuid")


def main(src, out):
    src, out = Path(src), Path(out)
    s = json.loads((src / "crawl_state.json").read_text(encoding="utf-8"))
    raw = src / "raw"
    stored = [m for m in s["stored"] if (raw / f"{m}.match.json.gz").exists() and (raw / f"{m}.timeline.json.gz").exists()]
    state = dict(ladder={k: s[k] for k in LADDER}, seen=s["seen"], stored=stored)
    d = out / "winprob" / s["patch"]
    d.mkdir(parents=True, exist_ok=True)
    (d / f"{s['platform']}.json").write_text(json.dumps(state), encoding="utf-8")
    (d / "raw.txt").write_bytes("".join(f"{m}.match.json.gz\n{m}.timeline.json.gz\n" for m in stored).encode())
    print(f"{s['platform']} {s['patch']}: {len(stored)} games ({len(s['stored']) - len(stored)} without both files left out), {len(s['seen'])} match ids seen,"
          f" player {s['next_puuid']} of {len(s['puuids'])}; state and file list in {d}")


if __name__ == "__main__":
    main(*sys.argv[1:3])
