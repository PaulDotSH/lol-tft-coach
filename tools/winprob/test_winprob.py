"""Offline tests for tools/winprob. py -3.12 -I tools/winprob/test_winprob.py

WINPROB_SKIP_CARGO=1 leaves out the one test that builds and runs the Rust replay example.
The synthetic fit and cargo are skipped while League of Legends runs.
"""
import argparse, collections, gzip, io, json, math, os, re, shutil, subprocess, sys, tempfile, tomllib, unittest
from contextlib import redirect_stdout
from pathlib import Path

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import crawl  # noqa: E402
import features as F  # noqa: E402
import fit  # noqa: E402
import riot_format as R  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
WINPROB_RS = REPO / "crates/coach/src/winprob.rs"
CONSTS_RS = REPO / "crates/coach/src/consts.rs"
FIGHTS_RS = REPO / "crates/coach/src/fights.rs"
LOLALYTICS_RS = REPO / "crates/data/src/lolalytics.rs"
HANDSET = REPO / "fixtures/coach/winprob_handset.toml"
SHIPPED = REPO / "crates/coach/winprob.toml"
DDRAGON_ITEMS = REPO / "fixtures/data/ddragon-item.json"


def league_running():
    try:
        out = subprocess.run(["tasklist"], capture_output=True, text=True, timeout=30).stdout
    except (OSError, subprocess.SubprocessError):
        return False
    return "League of Legends.exe" in out


def rust_const(text, name):
    m = re.search(rf"const {name}: [^=]+ = ([^;]+);", text)
    assert m, name
    return m.group(1).strip()


def rust_floats(s):
    return [float(x) for x in re.findall(r"-?\d+(?:\.\d+)?", s)]


class Parity(unittest.TestCase):
    """The Python features are the Rust ones: keys, order and constants, read from the Rust sources."""

    def test_keys(self):
        rs = WINPROB_RS.read_text(encoding="utf-8")
        m = re.search(r"pub const KEYS: \[&str; (\d+)\] =\s*\[(.*?)\];", rs, re.S)
        keys = re.findall(r'"(\w+)"', m.group(2))
        self.assertEqual(len(keys), int(m.group(1)))
        self.assertEqual(tuple(keys), F.KEYS)
        hand = tomllib.loads(HANDSET.read_text(encoding="utf-8"))
        for mode in ("rift", "aram"):
            self.assertTrue(set(hand[mode]) - {"metrics"} <= set(keys), mode)
            self.assertTrue(set(F.MODE_KEYS[mode]) <= set(keys), mode)
        self.assertEqual(set(hand["aram"]), set(F.MODE_KEYS["aram"]))
        self.assertEqual(tuple(tomllib.loads(SHIPPED.read_text(encoding="utf-8"))["knots"]), F.KNOTS)
        self.assertEqual(tuple(hand["knots"]), F.KNOTS)

    def test_constants(self):
        rs, consts, fights = (p.read_text(encoding="utf-8") for p in (WINPROB_RS, CONSTS_RS, FIGHTS_RS))
        self.assertEqual(float(rust_const(rs, "GOLD_PAD")), F.GOLD_PAD)
        self.assertEqual(float(rust_const(rs, "DEAD_CAP")), F.DEAD_CAP)
        self.assertEqual(rust_const(rs, "DRAKE_CAP"), "SOUL_DRAGONS - 1")
        self.assertEqual(int(rust_const(consts, "SOUL_DRAGONS")), F.SOUL_DRAGONS)
        self.assertEqual(F.DRAKE_CAP, F.SOUL_DRAGONS - 1)
        self.assertEqual(int(rust_const(consts, "GRUBS_COUNT")), F.GRUBS_COUNT)
        self.assertEqual(float(rust_const(consts, "BARON_BUFF")), F.BARON_BUFF)
        self.assertEqual(float(rust_const(consts, "ELDER_BUFF")), F.ELDER_BUFF)
        self.assertEqual(float(rust_const(consts, "INHIB_RESPAWN")), F.INHIB_RESPAWN["rift"])
        self.assertEqual(float(rust_const(consts, "INHIB_RESPAWN_ARAM")), F.INHIB_RESPAWN["aram"])
        self.assertEqual(float(rust_const(consts, "PLAN_SHRINK_K")), F.PLAN_SHRINK_K)
        self.assertEqual(tuple(rust_floats(rust_const(fights, "RESPAWN"))), F.RESPAWN)
        body = re.search(r"pub\(crate\) fn respawn\(.*?\n}\n", fights, re.S).group(0)
        growth = tuple((float(k), float(a), float(b)) for k, a, b in re.findall(r"([\d.]+) \* halves\(([\d.]+), ([\d.]+)\)", body))
        self.assertEqual(growth, F.RESPAWN_GROWTH)
        self.assertEqual(float(re.search(r"grow\.min\(([\d.]+)\)", body).group(1)), F.RESPAWN_GROWTH_MAX)
        self.assertIn("/ 30.0", body)
        self.assertEqual(tuple(int(x) for x in rust_floats(rust_const(LOLALYTICS_RS.read_text(encoding="utf-8"), "LENGTH_FROM"))), F.LENGTH_FROM)
        feats = re.search(r"pub fn features\(&self\).*?\n    }\n", rs, re.S).group(0)
        for snippet in ["((self.item_gold[0] + GOLD_PAD) / (self.item_gold[1] + GOLD_PAD)).ln()", "self.level[0] - self.level[1]",
                        "(self.cs[0] - self.cs[1]) / 100.0", "(mine - theirs) / (mine + theirs + 4.0).sqrt()", "diff(self.towers)",
                        "diff(self.inhibs)", "diff(self.drakes.map(|n| n.min(DRAKE_CAP)))", "self.soul as f32", "self.elder as f32",
                        "self.baron as f32", "diff(self.grubs) / GRUBS_COUNT as f32", "self.dead[1] - self.dead[0]", "self.outlook", "self.side as f32"]:
            self.assertIn(snippet, feats)
        self.assertIn("p.respawn_in.clamp(0.0, DEAD_CAP) / DEAD_CAP", rs)
        self.assertIn("i32::from(drakes[0] >= SOUL_DRAGONS) - i32::from(drakes[1] >= SOUL_DRAGONS)", rs)

    def test_respawn_and_shrunk_win_rates(self):
        self.assertEqual(F.respawn(1, 0.0), 10.0)
        self.assertAlmostEqual(F.respawn(10, 1200.0), 32.5 * (1 + 0.00425 * 10))
        self.assertAlmostEqual(F.respawn(18, 3300.0), 52.5 * 1.5)
        self.assertEqual(F.respawn(0, 0.0), 10.0)
        mine = [(f, 1000, w * 10) for f, w in zip((0, 20, 30, 40), (46, 50, 54, 58))]
        self.assertAlmostEqual(F.wr_from(mine, 15.0), 50 + 2 * 4000 / 4400)
        self.assertAlmostEqual(F.wr_from(mine, 31.0), F.shrink(100 * 1120 / 2000, 2000, 400))
        self.assertAlmostEqual(F.wr_from(mine, 99.0), F.shrink(58.0, 1000, 400))
        self.assertIsNone(F.wr_from([(0, 0, 0)], 5.0))

    def test_outlook_from_training_games_without_their_own_result(self):
        g = lambda mid, y, dur, blue, red: F.Game(mid, "rift", "16.21", y, [blue, red], dur, [], [])
        a, b = g("A", True, 22.0, [1, 2, 3, 4, 5], [6, 7, 8, 9, 10]), g("B", False, 33.0, [1, 12, 13, 14, 15], [16, 17, 18, 19, 20])
        t = dict(F.length_tables([a, b]))
        self.assertEqual(t[1], [[0, 0], [0, 0], [1, 1], [0, 0], [1, 0], [0, 0], [0, 0]], "champion 1: won at 22 min, lost at 33")
        c = g("C", True, 30.0, [1, 30, 31, 32, 33], [6, 34, 35, 36, 37])
        wr = lambda games, wins: F.shrink(100.0 * wins / games, games, F.PLAN_SHRINK_K)
        self.assertAlmostEqual(F.outlook(c, 10.0, t, own=False), (wr(2, 1) - 50) - (wr(1, 0) - 50))
        self.assertAlmostEqual(F.outlook(c, 31.0, t, own=False), (wr(1, 0) - 50) - 0, msg="after 30:00 only the 30-35 bucket and later count")
        self.assertAlmostEqual(F.outlook(a, 10.0, t, own=True), wr(1, 0) - 50, msg="A's own win is taken out; champion 1 keeps B's loss")
        self.assertEqual(F.outlook(g("D", True, 30.0, [40] * 5, [41] * 5), 10.0, t, own=False), 0.0, "no games, no outlook")

    def test_item_values_follow_the_app(self):
        v = F.item_values(json.loads(DDRAGON_ITEMS.read_text(encoding="utf-8")))
        self.assertEqual((v[1055], v[1036], v[3133], v[1001]), (450, 350, 1050, 300))
        self.assertEqual((v[2003], v[2055]), (0, 0), "consumables")
        self.assertNotIn(3340, v, "trinkets are not shop items")
        self.assertEqual(v[3042], 2900, "a transform of a shop item counts")
        self.assertEqual(v[3866], 400)
        self.assertNotIn(3867, v, "a transform of a transform is not loaded by ddragon::parse_items")


def participant(pid, champ, **flags):
    return {"participantId": pid, "teamId": 100 if pid <= 5 else 200, "championId": champ, "gameEndedInEarlySurrender": False,
            "teamEarlySurrendered": False, **{f"item{i}": 0 for i in range(7)}, **flags}


def hand_game(extra_events=(), **match_flags):
    """A ranked Rift game in the documented match-v5 shape: 12:30 long, blue wins."""
    ev = lambda ts, kind, **kw: {"type": kind, "timestamp": ts, **kw}
    buy = lambda ts, pid, item: ev(ts, R.ITEM_PURCHASED, participantId=pid, itemId=item)
    gone = lambda ts, pid, item: ev(ts, R.ITEM_DESTROYED, participantId=pid, itemId=item)
    events = [
        ev(0, "PAUSE_END"), buy(1000, 1, 1055), buy(1000, 1, 2003), buy(1500, 6, 1036), buy(1600, 6, 1036),
        ev(2000, "SKILL_LEVEL_UP", participantId=1, skillSlot=1), gone(30000, 1, 2003),
        buy(100000, 6, 3133), gone(100000, 6, 1036), gone(100000, 6, 1036),
        ev(105000, R.ITEM_UNDO, participantId=6, beforeId=3133, afterId=0, goldGain=1050),
        gone(110000, 6, 1036), gone(110000, 6, 1036), buy(110000, 6, 3133),
        ev(130000, R.ITEM_SOLD, participantId=1, itemId=1055), ev(131000, R.ITEM_UNDO, participantId=1, beforeId=0, afterId=1055, goldGain=-180),
        buy(140000, 1, 1001),
        ev(235000, R.CHAMPION_KILL, killerId=1, victimId=6, assistingParticipantIds=[2]),
        ev(250000, "WARD_PLACED", creatorId=3, wardType="YELLOW_TRINKET"),
        ev(300000, R.BUILDING_KILL, buildingType=R.TOWER, teamId=200, killerId=2, laneType="MID_LANE", towerType="OUTER_TURRET"),
        ev(330000, R.ELITE_MONSTER_KILL, monsterType=R.DRAGON, monsterSubType="FIRE_DRAGON", killerTeamId=200, killerId=7),
        ev(360000, R.BUILDING_KILL, buildingType=R.INHIBITOR, teamId=100, killerId=8, laneType="BOT_LANE"),
        ev(400000, R.ELITE_MONSTER_KILL, monsterType=R.GRUB, killerTeamId=100, killerId=2),
        ev(401000, R.ELITE_MONSTER_KILL, monsterType=R.GRUB, killerTeamId=100, killerId=2),
        ev(420000, R.ELITE_MONSTER_KILL, monsterType="RIFTHERALD", killerTeamId=200, killerId=7),
        ev(700000, R.CHAMPION_KILL, killerId=0, victimId=2, assistingParticipantIds=[]),
        ev(750000, R.GAME_END, gameId=1, winningTeam=100, realTimestamp=0),
        *extra_events,
    ]
    stamps = [m * 60000 for m in range(13)] + [750000]
    frames = []
    for ts in stamps:
        m = ts // 60000
        pf = {str(p): {"participantId": p, "level": 1 + m // 2 + (p == 1), "minionsKilled": 6 * m if p <= 5 else 5 * m,
                       "jungleMinionsKilled": 4 * m if p == 2 else 0, "totalGold": 500, "currentGold": 500, "xp": 0} for p in range(1, 11)}
        frames.append({"timestamp": ts, "participantFrames": pf, "events": []})
    for e in events:
        fi = max(i for i, ts in enumerate(stamps) if ts <= e["timestamp"])
        frames[min(fi + 1, len(frames) - 1)]["events"].append(e)
    parts = [participant(p, 100 + p) for p in range(1, 11)]
    for p in parts:
        p.update(match_flags)
    parts[0].update(item0=1055, item1=1001)
    parts[5].update(item0=3133)
    match = {"metadata": {"matchId": "EUW1_1"}, "info": {"gameVersion": "16.21.712.1234", "queueId": 420, "mapId": 11, "gameDuration": 750,
                                                         "participants": parts, "teams": [{"teamId": 100, "win": True}, {"teamId": 200, "win": False}]}}
    timeline = {"metadata": {"matchId": "EUW1_1"}, "info": {"frameInterval": 60000, "frames": frames,
                                                            "participants": [{"participantId": p, "puuid": f"p{p}"} for p in range(1, 11)]}}
    return match, timeline


class Timeline(unittest.TestCase):
    """features.py on a small hand-built timeline in the documented format."""

    @classmethod
    def setUpClass(cls):
        cls.values = F.item_values(json.loads(DDRAGON_ITEMS.read_text(encoding="utf-8")))

    def parse(self, *a, **kw):
        return F.parse_game(*hand_game(*a, **kw), self.values)

    def test_features_by_hand(self):
        g, why = self.parse()
        self.assertIsNone(why)
        self.assertEqual((g.mode, g.patch, g.y, g.champs), ("rift", "16.21", True, [[101, 102, 103, 104, 105], [106, 107, 108, 109, 110]]))
        self.assertEqual(g.minutes, [float(m) for m in range(13)], "one row per minute frame; the game-end frame is left out")
        x = lambda m: dict(zip(F.KEYS, g.states[m].features()))
        self.assertEqual(g.states[1].item_gold, [450, 700], "potion counts 0; two Long Swords")
        self.assertEqual(g.states[2].item_gold, [450, 1050], "the undone Caulfield's gave its swords back; bought again")
        f4 = x(4)
        self.assertEqual(g.states[4].item_gold, [750, 1050], "Doran's sold, sale undone, Boots bought")
        self.assertAlmostEqual(f4["gold"], math.log(3750 / 4050))
        self.assertAlmostEqual(f4["level"], 0.2)
        self.assertAlmostEqual(f4["cs"], (5 * 24 + 16 - 5 * 20) / 100)
        self.assertAlmostEqual(f4["kills"], 1 / math.sqrt(5))
        self.assertAlmostEqual(f4["dead"], 5 / 45, msg="level 2 at the 3:00 frame: 10 s, 5 left at 4:00")
        self.assertEqual(x(3)["dead"], 0.0)
        self.assertEqual(x(5)["dead"], 0.0)
        self.assertEqual((f4["towers"], f4["drakes"], f4["inhibs"], f4["grubs"], f4["side"]), (0, 0, 0, 0, 1))
        f6 = x(6)
        self.assertEqual((f6["towers"], f6["drakes"], f6["inhibs"], f6["grubs"]), (1, -1, -1, 0), "an inhibitor falling on the frame counts")
        self.assertAlmostEqual(x(7)["grubs"], 2 / 3)
        self.assertEqual(x(10)["inhibs"], -1)
        self.assertEqual(x(11)["inhibs"], 0, "back 300 s after it fell")
        self.assertEqual((x(12)["kills"], x(12)["dead"]), (1 / math.sqrt(5), 0.0), "an execution is a death, not a kill")
        self.assertEqual(g.final_items[1], collections.Counter({1055: 1, 1001: 1}))
        self.assertEqual(g.final_items[6], collections.Counter({3133: 1}))
        self.assertTrue(all(s.outlook == 0 and s.soul == s.elder == s.baron == 0 for s in g.states))

    def test_buffs_and_soul(self):
        ev = lambda ts, **kw: {"type": R.ELITE_MONSTER_KILL, "timestamp": ts, "killerId": 2, **kw}
        drakes = [ev(61000 * (i + 1), monsterType=R.DRAGON, monsterSubType="AIR_DRAGON", killerTeamId=100) for i in range(4)]
        soul = {"type": R.DRAGON_SOUL_GIVEN, "timestamp": 244000, "teamId": 100, "name": "Cloud"}
        more = drakes + [soul, ev(490000, monsterType=R.DRAGON, monsterSubType=R.ELDER, killerTeamId=200), ev(500000, monsterType=R.BARON, killerTeamId=100)]
        g, _ = self.parse(more)
        x = lambda m: dict(zip(F.KEYS, g.states[m].features()))
        self.assertEqual((x(3)["drakes"], x(3)["soul"]), (2, 0))
        self.assertEqual((x(5)["drakes"], x(5)["soul"]), (3, 1), "four dragons: capped at three, and the soul")
        self.assertEqual((x(9)["elder"], x(9)["baron"], x(9)["drakes"]), (-1, 1, 2), "an Elder is no dragon stack")
        self.assertEqual((x(11)["elder"], x(11)["baron"]), (0, 1), "Elder buff 150 s")
        self.assertEqual(x(12)["baron"], 0, "Baron buff 180 s")
        with self.assertRaises(F.FormatError):
            self.parse([dict(soul, teamId=200)])

    def test_left_out(self):
        self.assertEqual(self.parse(gameEndedInEarlySurrender=True), (None, "remake or early surrender"))

    def test_unknown_shapes_fail_loudly(self):
        for bad in [{"type": "NEW_EVENT", "timestamp": 5000}, {"timestamp": 5000}, {"type": R.CHAMPION_KILL, "timestamp": 5000, "victimId": 3},
                    {"type": R.BUILDING_KILL, "timestamp": 5000, "buildingType": "NEW_BUILDING", "teamId": 100},
                    {"type": R.ELITE_MONSTER_KILL, "timestamp": 5000, "monsterType": "NEW_MONSTER", "killerTeamId": 100},
                    {"type": R.ELITE_MONSTER_KILL, "timestamp": 5000, "monsterType": R.DRAGON, "monsterSubType": "NEW_DRAGON", "killerTeamId": 100},
                    {"type": R.ITEM_SOLD, "timestamp": 5000, "participantId": 3, "itemId": 3031},
                    {"type": R.ITEM_UNDO, "timestamp": 5000, "participantId": 3, "beforeId": 3031, "afterId": 0}]:
            with self.assertRaises(F.FormatError, msg=bad):
                self.parse([bad])
        match, timeline = hand_game()
        del timeline["info"]["frames"][3]["participantFrames"]["4"]["minionsKilled"]
        with self.assertRaises(F.FormatError):
            F.parse_game(match, timeline, self.values)
        match, timeline = hand_game()
        timeline["info"]["frameInterval"] = 30000
        with self.assertRaises(F.FormatError):
            F.parse_game(match, timeline, self.values)

    def test_rows_file_from_a_raw_dir(self):
        d = Path(tempfile.mkdtemp())
        try:
            for i, flags in enumerate([{}, {}, {"gameEndedInEarlySurrender": True}]):
                match, timeline = hand_game(**flags)
                match["metadata"]["matchId"] = timeline["metadata"]["matchId"] = f"EUW1_{i}"
                for name, obj in (("match", match), ("timeline", timeline)):
                    with gzip.open(d / f"EUW1_{i}.{name}.json.gz", "wt", encoding="utf-8") as f:
                        json.dump(obj, f)
            with redirect_stdout(io.StringIO()):
                F.main(["--raw", str(d), "--out", str(d / "rows.npz"), "--items", str(DDRAGON_ITEMS)])
            with np.load(d / "rows.npz") as z:
                r = {k: z[k] for k in z.files}
            self.assertEqual((str(r["mode"]), str(r["patch"]), str(r["source"]), tuple(r["keys"])), ("rift", "16.21", "riot", F.KEYS))
            self.assertEqual((r["X"].shape, list(r["match_ids"])), ((26, len(F.KEYS)), ["EUW1_0", "EUW1_1"]))
            self.assertEqual(list(r["test"]), [F.is_test(r["match_ids"][g]) for g in r["game"]])
            self.assertTrue(np.all(r["y"] == 1.0) and np.all(r["X"][:, F.KEYS.index("side")] == 1.0))
        finally:
            shutil.rmtree(d)

    def test_check_reports_the_item_rebuild(self):
        d = Path(tempfile.mkdtemp())
        try:
            match, timeline = hand_game()
            for name, obj in (("match", match), ("timeline", timeline)):
                with gzip.open(d / f"EUW1_1.{name}.json.gz", "wt", encoding="utf-8") as f:
                    json.dump(obj, f)
            with redirect_stdout(io.StringIO()) as out:
                self.assertTrue(F.check(d, 10, self.values))
            self.assertIn("10 of 10 players exact", out.getvalue())
        finally:
            shutil.rmtree(d)


class FakeApi:
    """Canned replies by path, in the shapes riot_format assumes; counts every request."""

    def __init__(self, replies):
        self.replies, self.platform, self.region, self.requests, self.calls = replies, "P", "G", 0, collections.Counter()

    def get(self, host, path, params=None):
        key = path + (f"?page={params['page']}" if params and "page" in params else "")
        self.requests += 1
        self.calls[key] += 1
        return self.replies.get(key)


class Crawl(unittest.TestCase):
    def test_limiter_waits_for_room(self):
        now, slept = [0.0], []
        tick = lambda: now[0]
        sleep = lambda d: (slept.append(d), now.__setitem__(0, now[0] + d))
        lim = crawl.Limiter(((3, 1.0), (5, 10.0)), clock=tick, sleep=sleep)
        for _ in range(6):
            lim.wait()
        self.assertEqual(len(slept), 2)
        self.assertAlmostEqual(slept[0], 1.0 + crawl.LIMIT_PAD)
        self.assertAlmostEqual(now[0], 10.0 + crawl.LIMIT_PAD, msg="the sixth request waits for the 10 s window")

    def test_resumes_without_fetching_twice(self):
        q = R.LEAGUE_QUEUE
        m = lambda mid, patch: {"metadata": {"matchId": mid}, "info": {"gameVersion": f"{patch}.1.2"}}
        replies = {
            R.LEAGUE_APEX["CHALLENGER"].format(queue=q): {"entries": [{"puuid": "a"}]},
            R.LEAGUE_APEX["MASTER"].format(queue=q): {"entries": [{"puuid": "b"}, {"puuid": "a"}]},
            R.LEAGUE_ENTRIES.format(queue=q, tier="DIAMOND", division="I") + "?page=1": [{"puuid": "c"}],
            R.MATCH_IDS.format(puuid="a"): ["M1", "M2"],
            R.MATCH_IDS.format(puuid="b"): ["M2", "M3"],
            R.MATCH_IDS.format(puuid="c"): ["M4", "M5"],
            **{R.MATCH.format(match_id=i): m(i, "16.20" if i == "M2" else "16.21") for i in ("M1", "M2", "M3", "M4", "M5")},
            **{R.TIMELINE.format(match_id=i): {"metadata": {"matchId": i}} for i in ("M1", "M3", "M4", "M5")},
        }
        d = Path(tempfile.mkdtemp())
        try:
            args = argparse.Namespace(platform="euw1", region="europe", queue=420, patch="16.21")
            path = d / "crawl_state.json"
            s = crawl.load_state(path, args)
            api = FakeApi(replies)
            with redirect_stdout(io.StringIO()):
                crawl.crawl(api, s, lambda: crawl.save_state(path, s), d, 2, "16.21", 420, 0)
            self.assertEqual(s["stored"], ["M1", "M3"])
            self.assertEqual(sorted(p.name for p in d.glob("*.gz")), ["M1.match.json.gz", "M1.timeline.json.gz", "M3.match.json.gz", "M3.timeline.json.gz"])
            self.assertEqual(api.calls[R.TIMELINE.format(match_id="M2")], 0, "a match on another patch costs no timeline")
            s = crawl.load_state(path, args)
            with redirect_stdout(io.StringIO()):
                crawl.crawl(api, s, lambda: crawl.save_state(path, s), d, 10, "16.21", 420, 0)
            self.assertEqual(s["stored"], ["M1", "M3", "M4", "M5"])
            self.assertTrue(all(n == 1 for k, n in api.calls.items() if "/matches/" in k and "by-puuid" not in k), api.calls)
            self.assertEqual(s["puuids"], ["a", "b", "c"])
            self.assertEqual(F.load_json(d / "M4.timeline.json.gz"), {"metadata": {"matchId": "M4"}})
            with self.assertRaises(SystemExit):
                crawl.load_state(path, argparse.Namespace(platform="na1", region="americas", queue=420, patch="16.21"))
        finally:
            shutil.rmtree(d)


TRUE = {
    "gold": [3.0, 5.0, 6.0, 6.5, 6.0, 5.0, 4.0],
    "level": [0.30, 0.50, 0.50, 0.50, 0.45, 0.40, 0.30],
    "cs": [-0.30, -0.30, -0.30, -0.25, -0.20, -0.20, -0.20],
    "kills": [0.30, 0.30, 0.25, 0.20, 0.20, 0.15, 0.10],
    "towers": [0.0, 0.15, 0.15, 0.20, 0.20, 0.25, 0.25],
    "inhibs": [0.0, 0.0, 0.0, 0.40, 0.50, 0.60, 0.80],
    "drakes": [0.10, 0.15, 0.15, 0.15, 0.15, 0.15, 0.15],
    "soul": [0.0, 0.0, 0.0, 0.50, 0.60, 0.70, 0.70],
    "elder": [0.0, 0.0, 0.0, 0.0, 1.00, 1.20, 1.20],
    "baron": [0.0, 0.0, 0.0, 0.80, 0.90, 1.00, 1.00],
    "grubs": [0.0] * 7,
    "dead": [0.20, 0.20, 0.30, 0.40, 0.50, 0.60, 0.70],
    "outlook": [0.05] * 7,
    "side": [0.15] * 7,
}
SYNTH_GAMES = 20000


def simulate(n_games, seed=1):
    """Games whose blue win chance at every minute is exactly σ(Σ β_k(t) x_k) with β = TRUE.
    The logit is a martingale walk; gold is set to whatever makes the sum come out."""
    rng = np.random.default_rng(seed)
    K = {k: i for i, k in enumerate(F.KEYS)}
    true = np.array([TRUE[k] for k in F.KEYS])
    game, minute, y, X, ids = [], [], [], [], []
    for gi in range(n_games):
        dur = int(np.clip(rng.normal(30, 6), 16, 50))
        T = dur + 1
        x = np.zeros((T, len(F.KEYS)))
        x[:, K["level"]] = np.cumsum(rng.normal(0, 0.12, T)) * (np.arange(T) > 0)
        x[:, K["cs"]] = np.cumsum(rng.normal(0, 0.08, T)) * (np.arange(T) > 0)
        km, kt = np.cumsum(rng.poisson(0.45, T)), np.cumsum(rng.poisson(0.45, T))
        km[0] = kt[0] = 0
        x[:, K["kills"]] = (km - kt) / np.sqrt(km + kt + 4)
        tw = np.cumsum(rng.poisson(0.22, (T, 2)) * (np.arange(T)[:, None] >= 11), axis=0)
        x[:, K["towers"]] = tw[:, 0] - tw[:, 1]
        drakes, soul = [0, 0], 0
        for t in range(T):
            if t >= 6 and t % 5 == 1 and soul == 0:
                drakes[int(rng.integers(2))] += 1
                soul = int(drakes[0] >= 4) - int(drakes[1] >= 4)
            x[t, K["drakes"]] = min(drakes[0], 3) - min(drakes[1], 3)
            x[t, K["soul"]] = soul
        for t in range(21, T):
            if rng.random() < 0.06:
                x[t:t + 5, K["inhibs"]] += rng.choice([-1, 1])
            if rng.random() < 0.07:
                x[t:t + 3, K["baron"]] = rng.choice([-1, 1])
            if soul and t >= 26 and rng.random() < 0.05:
                x[t:t + 2, K["elder"]] = rng.choice([-1, 1])
        x[7:, K["grubs"]] = (2 * rng.integers(0, 4) - 3) / 3
        x[1:, K["dead"]] = (rng.binomial(5, 0.08, T - 1) - rng.binomial(5, 0.08, T - 1)) * 0.6
        x[:, K["outlook"]] = rng.normal(0, 4) + rng.normal(0, 0.1) * np.arange(T)
        x[:, K["side"]] = 1.0
        H = fit.hat(np.arange(T, dtype=float))
        B = H @ true.T
        others = (B * x).sum(axis=1)
        L = np.empty(T)
        L[0] = others[0]
        for t in range(1, T):
            p = 1 / (1 + math.exp(-L[t - 1]))
            up, down = L[t - 1] + 0.35, L[t - 1] - 0.35
            pu, pd = 1 / (1 + math.exp(-up)), 1 / (1 + math.exp(-down))
            L[t] = up if rng.random() < (p - pd) / (pu - pd) else down
        x[1:, K["gold"]] = (L[1:] - others[1:]) / B[1:, K["gold"]]
        win = rng.random() < 1 / (1 + math.exp(-L[-1]))
        mid = f"SYN_{gi}"
        ids.append(mid)
        game += [gi] * T
        minute += list(range(T))
        y += [float(win)] * T
        X.append(x)
    test = np.array([F.is_test(ids[g]) for g in game])
    folds = np.array([F.fold(ids[g]) for g in game], np.int8)
    return dict(game=np.array(game, np.int64), minute=np.array(minute, float), y=np.array(y), X=np.concatenate(X), test=test, fold=folds,
                match_ids=np.array(ids), keys=np.array(F.KEYS), train_games=np.int64(sum(not F.is_test(m) for m in ids)))


SKIP_HEAVY = league_running()


@unittest.skipIf(SKIP_HEAVY, "League of Legends is running")
class SyntheticFit(unittest.TestCase):
    """fit.py on simulated games from a known table: it finds the table, is calibrated, and its
    TOML and cases load the way the Rust side reads them."""

    @classmethod
    def setUpClass(cls):
        cls.dir = Path(tempfile.mkdtemp(prefix="winprob-test-"))
        cls.rows = cls.dir / "rows.npz"
        cls.arrays = simulate(SYNTH_GAMES)
        F.save_rows(cls.rows, cls.arrays, "rift", "16.21", "synthetic")
        cls.toml, cls.cases = cls.dir / "winprob.toml", cls.dir / "winprob_cases.json"
        with redirect_stdout(io.StringIO()) as out:
            cls.status = fit.main(["--rows", str(cls.rows), "--out", str(cls.toml), "--cases", str(cls.cases)])
        cls.report = out.getvalue()
        print(cls.report, file=sys.stderr)

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.dir, ignore_errors=True)

    def test_gates_pass_and_files_are_written(self):
        self.assertEqual(self.status, 0, self.report)
        self.assertTrue(self.toml.exists() and self.cases.exists())
        model = tomllib.loads(self.toml.read_text(encoding="utf-8"))
        fit.check_schema(model)
        self.assertEqual((model["patch"], model["games"], tuple(model["knots"])), ("16.21", int(self.arrays["train_games"]), F.KNOTS))
        m = model["rift"]["metrics"]
        self.assertLessEqual(m["ece"], fit.ECE_MAX)
        self.assertLess(m["log_loss"], m["baseline_log_loss"])
        self.assertEqual(len(m["accuracy"]), 4)
        self.assertNotIn("aram", {k for k, v in model.items() if v})

    def test_recovers_the_coefficients(self):
        model = tomllib.loads(self.toml.read_text(encoding="utf-8"))
        table = model["rift"]
        a, test = self.arrays, self.arrays["test"]
        H = fit.hat(a["minute"][test])
        X = a["X"][test]
        lines = []
        for i, k in enumerate(F.KEYS):
            truth = fit.mean_beta(X[:, [i]], H, np.array([TRUE[k]]))[0]
            if k not in table:
                lines.append(f"{k}: dropped (true {truth:+.3f})")
                self.assertEqual(k, "grubs", f"{k} has a real effect ({truth:+.3f}) and was dropped\n{self.report}")
                continue
            got = fit.mean_beta(X[:, [i]], H, np.array([table[k]]))[0]
            lines.append(f"{k}: fitted {got:+.4f} true {truth:+.4f}")
            if k == "grubs":
                self.assertLess(abs(got), 0.1)
            else:
                self.assertEqual(np.sign(got), np.sign(truth), k)
                self.assertLess(abs(got - truth), 0.25 * abs(truth), f"{k}: {got} vs {truth}")
        print("\n".join(lines), file=sys.stderr)

    def test_calibrated_on_held_out_games(self):
        model = tomllib.loads(self.toml.read_text(encoding="utf-8"))
        a, test = self.arrays, self.arrays["test"]
        cases = [dict(mode="rift", minute=float(t), x=dict(zip(F.KEYS, map(float, x)))) for t, x in zip(a["minute"][test], a["X"][test])]
        p = np.array([fit.case_p(model, c) for c in cases])
        self.assertLess(fit.ece(a["y"][test], p), fit.ECE_MAX)

    def test_cases_reproduce_from_the_toml(self):
        model = tomllib.loads(self.toml.read_text(encoding="utf-8"))
        cases = json.loads(self.cases.read_text(encoding="utf-8"))
        self.assertEqual(len(cases), fit.N_CASES)
        for c in cases:
            self.assertEqual(set(c), {"mode", "minute", "x", "p"})
            self.assertTrue(set(c["x"]) <= set(F.KEYS))
            table = model[c["mode"]]
            z = 0.0
            for k in F.KEYS:
                if k in table and k in c["x"]:
                    z += fit.beta(model["knots"], table[k], c["minute"]) * c["x"][k]
            self.assertLess(abs(1 / (1 + math.exp(-z)) - c["p"]), 1e-4)
            self.assertLess(abs(fit.case_p(model, c) - c["p"]), 1e-12)

    def test_hat_basis_is_the_rust_interpolation(self):
        rng = np.random.default_rng(3)
        values = list(rng.normal(size=len(F.KNOTS)))
        for minute in [0.0, 3.3, 10.0, 12.5, 39.9, 40.0, 55.0, -1.0]:
            self.assertAlmostEqual(float(fit.hat(np.array([minute]))[0] @ values), fit.beta(list(F.KNOTS), values, minute))

    def test_synthetic_rows_never_reach_the_repo(self):
        with self.assertRaises(SystemExit), redirect_stdout(io.StringIO()):
            fit.main(["--rows", str(self.rows)])
        with self.assertRaises(SystemExit), redirect_stdout(io.StringIO()):
            fit.main(["--rows", str(self.rows), "--out", str(self.dir / "x.toml")])

    def test_another_mode_is_kept_when_rewriting(self):
        out = self.dir / "merged.toml"
        shutil.copy(self.toml, out)
        text = out.read_text(encoding="utf-8").replace("[aram]\n", "[aram]\ngold = [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]\n")
        out.write_text(text, encoding="utf-8")
        with redirect_stdout(io.StringIO()):
            self.assertEqual(fit.main(["--rows", str(self.rows), "--out", str(out), "--cases", str(self.dir / "merged.json")]), 0)
        merged = tomllib.loads(out.read_text(encoding="utf-8"))
        self.assertEqual(merged["aram"], {"gold": [1.0] * 7})
        self.assertEqual(merged["rift"], tomllib.loads(self.toml.read_text(encoding="utf-8"))["rift"])

    @unittest.skipIf(os.environ.get("WINPROB_SKIP_CARGO") or not shutil.which("cargo"), "cargo not run")
    def test_rust_replay_reads_the_toml(self):
        env = dict(os.environ, CARGO_BUILD_BUILD_DIR=str(REPO / "target-build"), CARGO_BUILD_JOBS="2")
        r = subprocess.run(["cargo", "run", "-q", "-p", "lol-coach", "--example", "winprob_replay", "--", str(self.toml)], cwd=REPO, env=env,
                           capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=3600)
        print(r.stdout[-3000:], r.stderr[-3000:], file=sys.stderr)
        self.assertEqual(r.returncode, 0, r.stderr[-3000:])
        self.assertIn(f"table: {self.toml}", r.stdout)
        self.assertIn("reliability over their", r.stdout)
        self.assertNotIn("panicked", r.stderr)


if __name__ == "__main__":
    unittest.main()
