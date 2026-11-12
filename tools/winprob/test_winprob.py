"""Offline tests for tools/winprob. py -3.12 -I tools/winprob/test_winprob.py

WINPROB_SKIP_CARGO=1 leaves out the one test that builds and runs the Rust replay example.
The synthetic fit and cargo are skipped while League of Legends runs.
"""
import argparse, collections, gzip, io, json, math, os, re, shutil, subprocess, sys, tempfile, threading, time, tomllib, types, unittest
from contextlib import redirect_stdout
from pathlib import Path

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import crawl  # noqa: E402
import features as F  # noqa: E402
import fit  # noqa: E402
import nightly  # noqa: E402
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

    def test_what_real_timelines_do(self):
        """Shapes seen in 16.22 replies (2026-11-12) that the documentation does not mention."""
        ev = lambda ts, kind, **kw: {"type": kind, "timestamp": ts, **kw}
        gone = lambda ts, pid, item: ev(ts, R.ITEM_DESTROYED, participantId=pid, itemId=item)
        atlas = [ev(0, R.ITEM_PURCHASED, participantId=0, itemId=3865), ev(0, R.ITEM_PURCHASED, participantId=0, itemId=3865)]
        more = atlas + [
            gone(70000, 5, 3865), gone(190000, 5, 3866), gone(200000, 5, 3867), gone(250000, 10, 3865),
            ev(130000, R.DRAGON_SOUL_GIVEN, teamId=0, name="Cloud"),
            ev(150000, R.ITEM_UNDO, participantId=2, beforeId=0, afterId=0, goldGain=75),
            ev(160000, R.ITEM_SOLD, participantId=3, itemId=3031),
            ev(170000, R.ITEM_UNDO, participantId=3, beforeId=3031, afterId=0, goldGain=3400),
            ev(300000, R.ITEM_PURCHASED, participantId=4, itemId=3070), gone(500000, 4, 3070),
            ev(500000, R.ITEM_PURCHASED, participantId=4, itemId=3004), gone(500000, 4, 3004),
            ev(410000, R.ELITE_MONSTER_KILL, monsterType=R.GRUB, killerTeamId=300, killerId=0),
            ev(410000, R.ELITE_MONSTER_KILL, monsterType="RIFTHERALD", killerTeamId=300, killerId=0),
        ]
        g, why = self.parse(more)
        self.assertIsNone(why)
        gold = lambda m: g.states[m].item_gold
        self.assertEqual(gold(1), [450 + 400, 700 + 400], "each support's World Atlas, bought by participant 0 at 0:00")
        self.assertEqual(gold(2), [450 + 400, 1050 + 400], "Runic Compass follows it with no event and counts the same")
        self.assertEqual(g.final_items[5], collections.Counter({3869: 1}), "the upgrade is not logged: the lowest of the five, same cost")
        self.assertEqual(g.final_items[10], collections.Counter({3866: 1}))
        self.assertEqual(g.final_items[4], collections.Counter({3042: 1}), "a Manamune bought with a full Tear is a Muramana at once")
        self.assertEqual((gold(12)[0], g.states[12].soul), (750 + 400 + 2900, 0), "Bounty of Worlds counts 0 in the app, the upgrade 400 again")
        self.assertEqual(g.states[12].grubs, [2, 0], "a grub that left the map (team 300) is nobody's")
        self.assertEqual(g.odd, collections.Counter({"undo with no item": 1, "lost an item of value never given": 1, "undid a purchase never seen": 1}))
        role = participant(4, 104, roleBoundItem=1001)
        self.assertEqual(F.final_items(role), collections.Counter({1001: 1}))

    def test_viego_keeps_his_items_through_a_possession(self):
        """Seen 2026-11-12: a possession destroys his whole inventory, then the body's, and gives nothing back."""
        ev = lambda ts, kind, pid, item: {"type": kind, "timestamp": ts, "participantId": pid, "itemId": item}
        match, timeline = hand_game([
            ev(200000, R.ITEM_PURCHASED, 3, 3031), ev(205000, R.ITEM_PURCHASED, 3, 1036),
            ev(260000, R.ITEM_DESTROYED, 3, 3031), ev(260000, R.ITEM_DESTROYED, 3, 1036),
            ev(270000, R.ITEM_DESTROYED, 3, 3865), ev(270000, R.ITEM_DESTROYED, 3, 1055),
            ev(290000, R.ITEM_PURCHASED, 3, 3133), ev(290000, R.ITEM_DESTROYED, 3, 1036),
        ])
        match["info"]["participants"][2]["championId"] = R.POSSESSOR
        g, why = F.parse_game(match, timeline, self.values)
        self.assertIsNone(why)
        self.assertEqual(g.final_items[3], collections.Counter({3031: 1, 3133: 1}), "only what a purchase of his used up is gone")
        self.assertEqual(g.odd, collections.Counter())

    def test_left_out(self):
        self.assertEqual(self.parse(gameEndedInEarlySurrender=True), (None, "remake or early surrender"))

    def test_unknown_shapes_fail_loudly(self):
        for bad in [{"type": "NEW_EVENT", "timestamp": 5000}, {"timestamp": 5000}, {"type": R.CHAMPION_KILL, "timestamp": 5000, "victimId": 3},
                    {"type": R.BUILDING_KILL, "timestamp": 5000, "buildingType": "NEW_BUILDING", "teamId": 100},
                    {"type": R.ELITE_MONSTER_KILL, "timestamp": 5000, "monsterType": "NEW_MONSTER", "killerTeamId": 100},
                    {"type": R.ELITE_MONSTER_KILL, "timestamp": 5000, "monsterType": R.BARON, "killerTeamId": 300},
                    {"type": R.ELITE_MONSTER_KILL, "timestamp": 5000, "monsterType": R.DRAGON, "monsterSubType": "NEW_DRAGON", "killerTeamId": 100},
                    {"type": R.ITEM_SOLD, "timestamp": 5000, "participantId": 11, "itemId": 3031},
                    {"type": R.ITEM_PURCHASED, "timestamp": 5000, "participantId": 0, "itemId": 3031},
                    {"type": R.ITEM_UNDO, "timestamp": 5000, "participantId": 3, "beforeId": 3031, "afterId": 1001}]:
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

    def crawl_one_player(self, ids, patches, **kw):
        q = R.LEAGUE_QUEUE
        replies = {
            R.LEAGUE_APEX["CHALLENGER"].format(queue=q): {"entries": [{"puuid": "top"}]},
            R.LEAGUE_ENTRIES.format(queue=q, tier="DIAMOND", division="I") + "?page=1": [{"puuid": "a"}],
            R.MATCH_IDS.format(puuid="a"): ids,
            **{R.MATCH.format(match_id=i): {"metadata": {"matchId": i}, "info": {"gameVersion": f"{p}.1.2"}} for i, p in zip(ids, patches)},
            **{R.TIMELINE.format(match_id=i): {"metadata": {"matchId": i}} for i in ids},
        }
        d = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, d)
        path = d / "crawl_state.json"
        args = argparse.Namespace(platform="euw1", region="europe", queue=420, patch="16.21")
        now = [0.0]

        class Ticking(FakeApi):
            def get(self, host, path, params=None):
                now[0] += 1
                return super().get(host, path, params)

        api = Ticking(replies)

        def run(**more):
            s = crawl.load_state(path, args)
            with redirect_stdout(io.StringIO()):
                crawl.crawl(api, s, lambda: crawl.save_state(path, s), d, 10, "16.21", 420, 0, clock=lambda: now[0], apex=False, **more)
            return s

        return api, run, run(**kw)

    def test_stops_at_its_hour_and_resumes(self):
        api, run, s = self.crawl_one_player(["M1", "M2", "M3", "M4"], ["16.21"] * 4, until=5.0)
        self.assertEqual((s["stored"], s["next_puuid"]), (["M1", "M2"], 0), "the fifth request is the last one started")
        self.assertEqual(run()["stored"], ["M1", "M2", "M3", "M4"])
        self.assertTrue(all(n == 1 for k, n in api.calls.items() if "/matches/" in k and "by-puuid" not in k), api.calls)

    def test_an_older_patch_ends_a_players_list(self):
        api, _, s = self.crawl_one_player(["M1", "M2", "M3", "M4", "M5"], ["16.22", "16.21", "16.20", "16.20", "16.21"])
        self.assertEqual(s["stored"], ["M2"], "ids come newest first: a newer patch is passed over, nothing follows an older one")
        self.assertEqual([api.calls[R.MATCH.format(match_id=i)] for i in ("M1", "M3", "M4", "M5")], [1, 1, 0, 0])
        self.assertEqual(s["next_puuid"], 1)

    def test_the_top_leagues_can_be_left_out(self):
        api, _, s = self.crawl_one_player(["M1"], ["16.21"])
        self.assertEqual((s["puuids"], s["apex_done"]), (["a"], []))
        self.assertFalse(any("leagues/by-queue" in k for k in api.calls), "Challenger, Grandmaster and Master are not read")
        self.assertEqual(sorted(k.split("/entries/")[1] for k in api.calls if "/entries/" in k and k.endswith("page=1")),
                         sorted(f"{R.LEAGUE_QUEUE}/{t}/{d}?page=1" for t in R.LADDER_TIERS for d in R.LADDER_DIVISIONS), "every division in turn")

    def test_players_read_with_another_key_are_read_again(self):
        q = R.LEAGUE_QUEUE
        replies = {
            R.LEAGUE_ENTRIES.format(queue=q, tier="DIAMOND", division="I") + "?page=1": [{"puuid": "new"}],
            R.MATCH_IDS.format(puuid="new"): ["M1", "M2"],
            R.MATCH.format(match_id="M2"): {"metadata": {"matchId": "M2"}, "info": {"gameVersion": "16.21.1.2"}},
            R.TIMELINE.format(match_id="M2"): {"metadata": {"matchId": "M2"}},
        }

        class Rekeyed(FakeApi):
            old = ("old",)

            def get(self, host, path, params=None):
                if path in [R.MATCH_IDS.format(puuid=p) for p in self.old]:
                    raise crawl.OtherKey(path)
                return super().get(host, path, params)

        d = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, d)

        def state():
            s = crawl.load_state(d / "crawl_state.json", argparse.Namespace(platform="euw1", region="europe", queue=420, patch="16.21"))
            s.update(puuids=["gone", "old"], next_puuid=1, apex_done=["CHALLENGER"], turn=3, exhausted=["DIAMOND I"], stored=["M1"], seen=["M1"])
            s["pages"]["DIAMOND I"] = 9
            return s

        s, api, out = state(), Rekeyed(replies), io.StringIO()
        with redirect_stdout(out):
            crawl.crawl(api, s, lambda: None, d, 10, "16.21", 420, 0, apex=False)
        self.assertEqual((s["puuids"], s["stored"]), (["new"], ["M1", "M2"]), "the ladder is read again, the games on disk are kept")
        self.assertEqual(api.calls[R.MATCH.format(match_id="M1")], 0, "a game already seen is not fetched again")
        self.assertIn("another key", out.getvalue())
        s, api = state(), Rekeyed(replies)
        api.old = ("old", "new")
        with redirect_stdout(io.StringIO()), self.assertRaises(crawl.OtherKey, msg="not twice in one run"):
            crawl.crawl(api, s, lambda: None, d, 10, "16.21", 420, 0, apex=False)

    def test_a_player_id_of_another_key_is_not_any_bad_request(self):
        api = crawl.Api("k", "euw1", "europe")

        def reply(text):
            def fail():
                raise crawl.requests.HTTPError("400")
            return types.SimpleNamespace(status_code=400, text=text, headers={}, raise_for_status=fail)

        said = iter(['{"status":{"message":"Bad Request - Exception decrypting abc","status_code":400}}', '{"status":{"message":"Bad Request","status_code":400}}'])
        api.s = types.SimpleNamespace(get=lambda url, params=None, timeout=None: reply(next(said)))
        with self.assertRaises(crawl.OtherKey):
            api.get(api.region, "/x")
        with self.assertRaises(crawl.requests.HTTPError):
            api.get(api.region, "/x")

    def test_a_dropped_connection_is_tried_again(self):
        api = crawl.Api("k", "euw1", "europe")
        ok = types.SimpleNamespace(status_code=200, text="{}", headers={}, raise_for_status=lambda: None, json=lambda: {"a": 1})
        replies = iter([crawl.requests.ConnectionError("Connection aborted."), crawl.requests.Timeout("read timed out"), ok])

        def get(url, params=None, timeout=None):
            r = next(replies)
            if isinstance(r, Exception):
                raise r
            return r

        api.s, slept = types.SimpleNamespace(get=get), []
        real, crawl.time.sleep = crawl.time.sleep, slept.append
        try:
            self.assertEqual(api.get(api.region, "/x"), {"a": 1})
            self.assertEqual(len(slept), 2, "a wait before each new try")
            api.s = types.SimpleNamespace(get=lambda url, params=None, timeout=None: (_ for _ in ()).throw(crawl.requests.ConnectionError("down")))
            with self.assertRaises(RuntimeError, msg="not for ever"):
                api.get(api.region, "/x")
        finally:
            crawl.time.sleep = real

    def test_key_from_a_file_or_the_environment(self):
        d = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, d)
        (d / "key.txt").write_text("﻿RGAPI-file\r\n", encoding="utf-8")
        self.assertEqual(crawl.read_key(d / "key.txt", {"RIOT_API_KEY": "RGAPI-env"}), "RGAPI-file", "a file named on the command line wins")
        self.assertEqual(crawl.read_key(None, {"RIOT_API_KEY": " RGAPI-env "}, d / "none.txt"), "RGAPI-env")
        self.assertEqual(crawl.read_key(None, {}, d / "key.txt"), "RGAPI-file")
        with self.assertRaises(SystemExit):
            crawl.read_key(None, {}, d / "none.txt")
        with self.assertRaises(SystemExit):
            crawl.read_key(d / "none.txt", {"RIOT_API_KEY": "RGAPI-env"})


class Nightly(unittest.TestCase):
    def setUp(self):
        self.d = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.d)
        self.calls, self.said, self.n, self.status = [], [], 0, {}

    def args(self, **kw):
        return argparse.Namespace(**{**dict(dir=self.d, hours=2.0, games=20000, fit_at=10, refit_every=5, keep=2, key_file=None, publish=None, servers=["euw1/europe"]), **kw})

    def state(self, d, patch, n, platform="euw1"):
        d.mkdir(parents=True, exist_ok=True)
        (d / "crawl_state.json").write_text(json.dumps({"patch": patch, "platform": platform, "stored": [f"M{i}" for i in range(n)]}), encoding="utf-8")

    def sh(self, cmd):
        script = f"git {cmd[3]}" if cmd[0] == "git" else Path(cmd[2]).name
        self.calls.append(cmd)
        if script == "crawl.py":
            platform = cmd[cmd.index("--platform") + 1]
            self.state(Path(cmd[cmd.index("--out") + 1]), "16.23", self.n[platform] if isinstance(self.n, dict) else self.n, platform)
        return self.status.get(script, 0)

    def run_once(self, n, playing=False, **kw):
        self.n, self.calls = n, []
        status = nightly.run(self.args(**kw), "16.23", self.sh, self.said.append, lambda: playing)
        return status, [f"git {c[3]}" if c[0] == "git" else Path(c[2]).name for c in self.calls]

    def memo(self):
        p = self.d / "16.23" / "nightly.json"
        return json.loads(p.read_text(encoding="utf-8")) if p.exists() else None

    def test_a_patch_gets_its_own_directory(self):
        self.assertEqual(nightly.patch_dir(self.d, "16.23"), self.d / "16.23")
        self.state(self.d, "16.22", 3)
        self.assertEqual(nightly.patch_dir(self.d, "16.22"), self.d, "a crawl started by hand in the root goes on there")
        self.assertEqual(nightly.patch_dir(self.d, "16.23"), self.d / "16.23")
        self.assertEqual((nightly.stored(self.d), nightly.stored(self.d / "16.23")), (3, 0))
        self.assertEqual((nightly.server_dir(self.d, "euw1"), nightly.server_dir(self.d, "kr")), (self.d, self.d / "kr"), "and so does its server")

    def test_one_server_for_each_rate_limit(self):
        self.assertEqual(nightly.servers(["euw1/europe", "na1/americas"]), [("euw1", "europe"), ("na1", "americas")])
        for bad in (["euw1/europe", "eun1/europe"], ["euw1"], []):
            with self.assertRaises(SystemExit, msg=bad):
                nightly.servers(bad)

    def test_servers_are_crawled_side_by_side_for_one_fit(self):
        names = ["euw1/europe", "na1/americas", "kr/asia"]
        together = threading.Barrier(3, timeout=20)

        def sh(cmd):
            if Path(cmd[2]).name == "crawl.py":
                together.wait()
            return self.sh(cmd)

        self.n = {"euw1": 4, "na1": 3, "kr": 2}
        self.assertEqual(nightly.run(self.args(servers=names), "16.23", sh, self.said.append, lambda: False), 0)
        d = self.d / "16.23"
        arg = lambda c, name: c[c.index(name) + 1]
        self.assertEqual(sorted((arg(c, "--platform"), arg(c, "--region"), arg(c, "--out"), arg(c, "--raw"), arg(c, "--games")) for c in self.calls),
                         sorted((p, r, str(d / p), str(d / "raw"), "6667") for p, r in (n.split("/") for n in names)), "one raw directory, a third of the games each")
        self.assertIsNone(self.memo(), "9 games in all: a fit takes 10")
        self.assertEqual(self.run_once({"euw1": 4, "na1": 4, "kr": 2}, servers=names), (0, ["crawl.py"] * 3 + ["features.py", "fit.py"]))
        self.assertEqual(self.memo()["fit_games"], 10)
        self.status = {"crawl.py": 1}
        self.assertEqual(self.run_once({"euw1": 4, "na1": 4, "kr": 2}, servers=names)[0], 1)

    def test_goes_on_without_a_break(self):
        now, slept, runs = [0.0], [], iter([100.0, 7200.0, None])

        def once():
            took = next(runs)
            if took is None:
                raise KeyboardInterrupt
            now[0] += took

        with self.assertRaises(KeyboardInterrupt):
            nightly.forever(once, pause=600.0, clock=lambda: now[0], sleep=slept.append)
        self.assertEqual(slept, [500.0, 0.0], "a run with nothing to do is not started again at once")

    def test_when_a_fit_is_due(self):
        due = lambda games, last: nightly.due(games, last, 10000, 2500)
        self.assertEqual([due(9999, None), due(10000, None), due(12499, 10000), due(12500, 10000)], [False, True, False, True])

    def test_crawls_then_fits_when_enough_games_are_in(self):
        self.assertEqual(self.run_once(9), (0, ["crawl.py"]))
        self.assertIsNone(self.memo())
        self.assertEqual(self.run_once(10), (0, ["crawl.py", "features.py", "fit.py"]))
        crawl_cmd, feat, fit_cmd = self.calls
        d = self.d / "16.23"
        self.assertEqual(crawl_cmd[:2] + crawl_cmd[3:], [sys.executable, "-I", "--games", "20000", "--hours", "2.0", "--patch", "16.23",
                                                         "--platform", "euw1", "--region", "europe", "--out", str(d / "euw1"), "--raw", str(d / "raw")])
        self.assertEqual(feat[3:], ["--raw", str(d / "raw"), "--out", str(d / "rows.npz"), "--patch", "16.23"])
        self.assertEqual(fit_cmd[3:], ["--rows", str(d / "rows.npz"), "--out", str(d / "fit" / "winprob.toml"), "--cases", str(d / "fit" / "winprob_cases.json")])
        self.assertEqual((self.memo()["fit_games"], self.memo()["passed"]), (10, True))
        self.assertEqual(self.run_once(14), (0, ["crawl.py"]), "not again until refit_every more games")
        self.assertEqual(self.run_once(15)[1], ["crawl.py", "features.py", "fit.py"])
        self.assertEqual(self.run_once(15, key_file="k.txt")[1], ["crawl.py"])
        self.assertEqual(self.calls[0][-2:], ["--key-file", "k.txt"])

    def test_a_failed_step_shows_in_the_exit_status(self):
        self.status = {"crawl.py": 1}
        self.assertEqual(self.run_once(10), (1, ["crawl.py", "features.py", "fit.py"]), "a rejected key does not stop the fit")
        self.assertTrue(self.memo()["passed"])
        self.status = {"fit.py": 1}
        self.assertEqual(self.run_once(15), (1, ["crawl.py", "features.py", "fit.py"]))
        self.assertEqual((self.memo()["fit_games"], self.memo()["passed"]), (15, False))
        self.status = {"features.py": 1}
        self.assertEqual(self.run_once(20), (1, ["crawl.py", "features.py"]))
        self.assertFalse(self.memo()["passed"])

    def test_no_fit_while_a_game_runs(self):
        self.assertEqual(self.run_once(10, playing=True), (0, ["crawl.py"]))
        self.assertIsNone(self.memo(), "the next run fits")
        self.assertIn("League", " ".join(self.said))

    def test_publishes_the_fitted_table(self):
        pub, table = self.d / "pub", self.d / "16.23" / "fit" / "winprob.toml"
        self.assertEqual(self.run_once(3, publish=str(pub)), (0, ["crawl.py"]))
        self.assertFalse(pub.exists(), "no table yet, nothing published")
        table.parent.mkdir()
        table.write_text('patch = "16.23"\ngames = 8000\n', encoding="utf-8")
        self.assertEqual(self.run_once(3, publish=str(pub)), (0, ["crawl.py"]), "a plain directory gets a copy")
        self.assertEqual((pub / "winprob.toml").read_text(encoding="utf-8"), table.read_text(encoding="utf-8"))
        (pub / ".git").mkdir()
        table.write_text('patch = "16.23"\ngames = 10500\n', encoding="utf-8")
        self.status = {"git diff": 1}
        self.assertEqual(self.run_once(3, publish=str(pub)), (0, ["crawl.py", "git add", "git diff", "git commit", "git push"]))
        self.assertEqual(self.calls[3], ["git", "-C", str(pub), "commit", "-q", "-m", "winprob 16.23: 10500 games"])
        self.assertIn("10500", (pub / "winprob.toml").read_text(encoding="utf-8"))
        self.status = {"git push": 1}
        self.assertEqual(self.run_once(3, publish=str(pub)), (1, ["crawl.py", "git add", "git diff", "git push"]), "nothing new to commit; the push is tried again")

    def test_old_patches_lose_their_raw_games_only(self):
        for name in ("16.9", "16.10", "16.11", "notes"):
            (self.d / name / "raw").mkdir(parents=True)
            (self.d / name / "fit").mkdir()
            for f in ("raw/M1.match.json.gz", "rows.npz", "fit/winprob.toml", "crawl_state.json"):
                (self.d / name / f).write_text("x", encoding="utf-8")
        (self.d / "raw").mkdir()
        self.assertEqual(nightly.prune(self.d, 2), ["16.9"])
        left = sorted(p.relative_to(self.d / "16.9").as_posix() for p in (self.d / "16.9").rglob("*") if p.is_file())
        self.assertEqual(left, ["crawl_state.json", "fit/winprob.toml"])
        for name in ("16.10", "16.11", "notes"):
            self.assertTrue((self.d / name / "raw" / "M1.match.json.gz").exists() and (self.d / name / "rows.npz").exists(), name)
        self.assertTrue((self.d / "raw").exists(), "the root's own crawl is never touched")
        self.assertEqual(nightly.prune(self.d, 2), [])
        self.assertEqual(nightly.prune(self.d, 0), [], "0 keeps everything")

    def test_one_run_at_a_time(self):
        path = self.d / "nightly.lock"
        with crawl.Lock(path):
            with self.assertRaises(crawl.Busy):
                crawl.Lock(path).__enter__()
        self.assertTrue(path.exists(), "the file a killed run leaves behind holds nobody up")
        with crawl.Lock(path):
            pass
        holder = "import sys, time; sys.path.insert(0, sys.argv[1]); import crawl; lock = crawl.Lock(sys.argv[2]); lock.__enter__(); print('held', flush=True); time.sleep(60)"
        held = subprocess.Popen([sys.executable, "-I", "-c", holder, str(Path(crawl.__file__).parent), str(path)], stdout=subprocess.PIPE, text=True)
        try:
            self.assertEqual(held.stdout.readline().strip(), "held")
            with self.assertRaises(crawl.Busy, msg="another process has it"):
                crawl.Lock(path).__enter__()
        finally:
            held.kill()
            held.wait()
            held.stdout.close()
        with crawl.Lock(path):
            pass


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


class Report(unittest.TestCase):
    def test_rows_by_gold_lead(self):
        lead = np.array([-0.30, -0.15, 0.0, 0.01, 0.07, 0.07, 0.50])
        y = np.array([0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0])
        p = np.array([0.1, 0.3, 0.5, 0.5, 0.6, 0.8, 0.9])
        got = fit.gold_slices(np.log1p(np.abs(lead)) * np.sign(lead), y, p)
        self.assertEqual([(s[0], s[1], s[2]) for s in got], [(-math.inf, -0.20, 1), (-0.20, -0.10, 1), (-0.02, 0.02, 2), (0.05, 0.10, 2), (0.20, math.inf, 1)])
        self.assertEqual([(round(s[3], 3), s[4]) for s in got][2:4], [(0.5, 0.5), (0.7, 0.5)])
        self.assertEqual([fit.lead_label(lo, hi) for lo, hi in ((-math.inf, -0.20), (-0.10, -0.05), (-0.02, 0.02), (0.05, 0.10), (0.20, math.inf))],
                         ["red +20 % or more", "red +5 to +10 %", "even (under 2 %)", "blue +5 to +10 %", "blue +20 % or more"])

    def test_what_the_model_says_of_its_favourite(self):
        got = fit.sides(np.array([1.0, 0.0, 0.0, 0.0]), np.array([0.7, 0.7, 0.3, 0.2]))
        self.assertEqual([(s, n, round(said, 3), won) for s, n, said, won in got], [("blue", 2, 0.7, 0.5), ("red", 2, 0.75, 1.0)])

    def test_whole_games_are_drawn_again(self):
        rng = np.random.default_rng(5)
        games, rows = 200, 30
        game = np.repeat(np.arange(games), rows)
        y = np.repeat((rng.random(games) < 0.6).astype(float), rows)
        p, base = np.full(games * rows, 0.6), np.full(games * rows, 0.5)
        spans = fit.bootstrap(game, y, p, base)
        self.assertEqual(spans, fit.bootstrap(game, y, p, base), "the same draws every time")
        lo, hi = spans["ece"]
        self.assertGreater(hi - lo, 2.0, "200 games are 200 results, however many rows each has")
        self.assertLessEqual(lo, fit.ece(y, p))
        self.assertLess(spans["log_loss"][0], spans["log_loss"][1])
        self.assertEqual(set(spans), {"ece", "log_loss", "gain"})
        self.assertEqual(set(fit.bootstrap(game, y, p)), {"ece", "log_loss"})
        one = fit.bootstrap(np.zeros(rows, int), np.ones(rows), np.full(rows, 0.6))
        self.assertAlmostEqual(one["ece"][0], one["ece"][1])


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

    def test_the_report_has_slices_sides_and_resampled_spans(self):
        for line in ("by blue's item gold lead", "red +20 % or more", "blue +5 to +10 %", "by favoured side", "favours blue", "favours red",
                     "blue won", "whole held-out games drawn again", "log loss under gold-only"):
            self.assertIn(line, self.report)

    def test_a_feature_can_be_kept_whatever_its_fold_signs(self):
        self.assertNotIn("grubs", tomllib.loads(self.toml.read_text(encoding="utf-8"))["rift"], "dropped by the sign rule in the plain fit")
        out, cases = self.dir / "kept.toml", self.dir / "kept.json"
        with redirect_stdout(io.StringIO()) as said:
            status = fit.main(["--rows", str(self.rows), "--out", str(out), "--cases", str(cases), "--keep", "grubs"])
        self.assertEqual(status, 0, said.getvalue())
        self.assertIn("grubs", tomllib.loads(out.read_text(encoding="utf-8"))["rift"])
        self.assertIn("KEPT (--keep)", said.getvalue())
        self.assertNotIn("DROPPED", "".join(line for line in said.getvalue().splitlines() if line.strip().startswith("grubs")))
        out.unlink()
        cases.unlink()
        with self.assertRaises(SystemExit), redirect_stdout(io.StringIO()):
            fit.main(["--rows", str(self.rows), "--out", str(out), "--cases", str(cases), "--keep", "nonsense"])

    def test_scores_a_table_on_games_it_was_not_fitted_on(self):
        other = self.dir / "other.npz"
        F.save_rows(other, simulate(3000, seed=2), "rift", "16.22", "synthetic")
        before = sorted(p.name for p in self.dir.iterdir())
        with redirect_stdout(io.StringIO()) as out:
            status = fit.main(["--score", str(self.toml), "--rows", str(other)])
        self.assertEqual(status, 0, out.getvalue())
        self.assertIn("3000 rift games of patch 16.22", out.getvalue())
        self.assertNotIn("gold-only", out.getvalue(), "no fit, so no gold-only model to compare with")
        self.assertEqual(sorted(p.name for p in self.dir.iterdir()), before, "scoring writes nothing")
        with np.load(other) as z:
            d = {k: z[k] for k in z.files}
        model = tomllib.loads(self.toml.read_text(encoding="utf-8"))
        good = fit.score(model, d)
        self.assertLess(good["overall"]["ece"], fit.ECE_MAX)
        self.assertEqual(good["fails"], [])
        sure = dict(model, rift={k: [2 * x for x in v] if k != "metrics" else v for k, v in model["rift"].items()})
        bad = fit.score(sure, d)
        self.assertGreater(bad["overall"]["ece"], 2 * good["overall"]["ece"])
        self.assertTrue(bad["fails"])
        ahead = [s for s in bad["gold"] if s[0] > 0][-1]
        self.assertGreater(ahead[3], ahead[4] + 0.02, "a table twice as sure says more for the gold leader than the leader wins")
        unfitted = self.dir / "unfitted.toml"
        unfitted.write_text(fit.toml_text("16.22", 0, F.KNOTS, {}), encoding="utf-8")
        with self.assertRaises(SystemExit), redirect_stdout(io.StringIO()):
            fit.main(["--score", str(unfitted), "--rows", str(other)])
        unfitted.unlink()

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
