"""Rows for the win-probability fit: one per minute frame of each crawled game, from blue's side,
with the features of `State::features` in crates/coach/src/winprob.rs.

py -3.12 -I tools/winprob/features.py [--raw DIR] [--out rows.npz] [--mode rift|aram] [--patch 16.22] [--items item.json]
py -3.12 -I tools/winprob/features.py --check [--raw DIR] [--limit N]

--check runs the plan T0b checks on crawled games (item gold rebuilt from the item events against
each player's final items, event types seen, frame spacing) and writes nothing.
"""
import argparse, collections, gzip, json, math, os, sys, zlib
from pathlib import Path


sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import riot_format as R  # noqa: E402
from riot_format import FormatError, need  # noqa: E402

DEV = Path(os.environ.get("LOCALAPPDATA") or os.environ.get("XDG_DATA_HOME") or Path.home() / ".local" / "share") / "lol-coach" / "dev" / "winprob"
RAW = DEV / "raw"
ROWS = DEV / "rows.npz"

KEYS = ("gold", "level", "cs", "kills", "towers", "inhibs", "drakes", "soul", "elder", "baron", "grubs", "dead", "outlook", "side")
MODE_KEYS = {"rift": KEYS, "aram": ("gold", "level", "kills", "towers", "inhibs", "dead", "outlook")}
KNOTS = (0, 10, 15, 20, 25, 30, 40)

GOLD_PAD = 3000.0
SOUL_DRAGONS = 4
CHECK_TEAM_MEAN = 0.02
CHECK_TEAM_WORST = 0.10
CHECK_NEXUS = 0.9
DRAKE_CAP = SOUL_DRAGONS - 1
DEAD_CAP = 45.0
GRUBS_COUNT = 3
BARON_BUFF = 180.0
ELDER_BUFF = 150.0
INHIB_RESPAWN = {"rift": 300.0, "aram": 250.0}
RESPAWN = (10.0, 10.0, 12.0, 12.0, 14.0, 16.0, 20.0, 25.0, 28.0, 32.5, 35.0, 37.5, 40.0, 42.5, 45.0, 47.5, 50.0, 52.5)
RESPAWN_GROWTH = ((0.00425, 900.0, 1800.0), (0.003, 1800.0, 2700.0), (0.0145, 2700.0, 3300.0))
RESPAWN_GROWTH_MAX = 0.5
PLAN_SHRINK_K = 400.0
LENGTH_FROM = (0, 15, 20, 25, 30, 35, 40)
MIN_GAME_S = 300
TEST_SHARE = 5
FOLDS = 5


def respawn(level, t):
    """Seconds dead at `level` dying at game time `t` (s): `fights::respawn`."""
    base = RESPAWN[min(max(level, 1), 18) - 1]
    grow = sum(k * max((min(t, to) - start) / 30.0, 0.0) for k, start, to in RESPAWN_GROWTH)
    return base * (1.0 + min(grow, RESPAWN_GROWTH_MAX))


def shrink(wr, games, k):
    return 50.0 + (wr - 50.0) * games / (games + k)


def wr_from(buckets, minute):
    """`winprob::wr_from`: shrunk win rate (percent) over the length buckets still ahead at `minute`;
    `buckets` is [(from_min, games, wins)]. None without games."""
    games = wins = 0
    for i, (_, g, w) in enumerate(buckets):
        if i + 1 == len(buckets) or buckets[i + 1][0] > minute:
            games, wins = games + g, wins + w
    return shrink(100.0 * wins / games, games, PLAN_SHRINK_K) if games > 0 else None


def is_test(match_id):
    return zlib.crc32(f"split:{match_id}".encode()) % TEST_SHARE == 0


def fold(match_id):
    return zlib.crc32(f"fold:{match_id}".encode()) % FOLDS


class State:
    """`winprob::State`: pairs are [blue, red]; `inhibs` is [red's down, blue's down]."""

    def __init__(self):
        self.item_gold = [0.0, 0.0]
        self.level = [0.0, 0.0]
        self.cs = [0.0, 0.0]
        self.kills = [0.0, 0.0]
        self.towers = [0, 0]
        self.inhibs = [0, 0]
        self.drakes = [0, 0]
        self.soul = self.elder = self.baron = 0
        self.grubs = [0, 0]
        self.dead = [0.0, 0.0]
        self.outlook = 0.0
        self.side = 1

    def features(self):
        diff = lambda v: float(v[0] - v[1])
        mine, theirs = self.kills
        return [
            math.log((self.item_gold[0] + GOLD_PAD) / (self.item_gold[1] + GOLD_PAD)),
            self.level[0] - self.level[1],
            (self.cs[0] - self.cs[1]) / 100.0,
            (mine - theirs) / math.sqrt(mine + theirs + 4.0),
            diff(self.towers),
            diff(self.inhibs),
            diff([min(n, DRAKE_CAP) for n in self.drakes]),
            float(self.soul),
            float(self.elder),
            float(self.baron),
            diff(self.grubs) / GRUBS_COUNT,
            self.dead[1] - self.dead[0],
            self.outlook,
            float(self.side),
        ]


def item_values(ddragon):
    """Item id → gold it adds to an inventory, as the app counts it: `ddragon::parse_items`' shop
    filter plus transforms, at full cost except consumables (`common::held_value`); others 0."""
    raw = {int(k): v for k, v in ddragon["data"].items() if k.isdigit()}
    maps = lambda d: d.get("maps", {})
    shop = lambda i, d: (i < 200_000 and d["gold"]["purchasable"] and (maps(d).get("11") is True or maps(d).get("12") is True)
                         and d.get("requiredChampion") is None and d.get("requiredAlly") is None and d.get("inStore") is not False
                         and d.get("hideFromAll") is not True and "Trinket" not in d.get("tags", []))
    keep = {i for i, d in raw.items() if shop(i, d)}
    keep |= {i for i, d in raw.items() if d.get("specialRecipe") in keep}
    values = Values({i: 0 if "Consumable" in raw[i].get("tags", []) else raw[i]["gold"]["total"] for i in keep})
    for i, d in sorted(raw.items(), reverse=True):
        only = [int(x) for x in d.get("from", [])]
        if d.get("specialRecipe"):
            values.after[int(d["specialRecipe"])] = i
            values.at_once.add(int(d["specialRecipe"]))
        elif len(only) == 1 and d["gold"]["base"] == 0:
            values.after[only[0]] = i
    return values


class Values(dict):
    """Item id → gold. `after`: the item one turns into with no purchase event (a quest item's
    next stage, the support item's upgrade, upgraded boots); of several, the lowest id, all of
    which cost the same. `at_once`: those of them that can turn the moment they are bought (a Tear
    item bought with the Tear full)."""

    def __init__(self, *a):
        super().__init__(*a)
        self.after = {}
        self.at_once = set()


class Inventories:
    """Held items per participant from the item events. An undone purchase also gives back the
    items destroyed by that participant at the purchase's timestamp (its components). An item
    destroyed with no purchase at that moment, or bought at that very moment (a Tear item finished
    as it is bought), becomes `Values.after` of it; for `possessors`
    (Viego, whose possessions destroy all he holds and never give it back) it is skipped. Real timelines also
    lose items the events never gave (components of an item bought whole, rune and quest items):
    those are counted in `odd`, not errors; `check` measures what they cost."""

    def __init__(self, values, bought_at=(), possessors=()):
        self.values = values
        self.after = getattr(values, "after", {})
        self.at_once = getattr(values, "at_once", ())
        self.bought_at = collections.defaultdict(set)
        for pid, ts, item in bought_at:
            self.bought_at[pid, ts].add(item)
        self.possessors = set(possessors)
        self.held = collections.defaultdict(collections.Counter)
        self.buys = collections.defaultdict(list)
        self.destroyed = collections.defaultdict(list)
        self.odd = collections.Counter()

    def _remove(self, pid, item, where):
        if self.held[pid][item] > 0:
            self.held[pid][item] -= 1
        elif self.values.get(item, 0) > 0:
            self.odd["lost an item of value never given"] += 1

    def apply(self, ev, where):
        pid, kind, ts = ev[R.E_PARTICIPANT], ev[R.E_TYPE], ev[R.E_TIMESTAMP]
        if kind == R.ITEM_PURCHASED:
            self.held[pid][ev[R.E_ITEM]] += 1
            self.buys[pid].append((ev[R.E_ITEM], ts))
        elif kind == R.ITEM_SOLD:
            self._remove(pid, ev[R.E_ITEM], where)
        elif kind == R.ITEM_DESTROYED:
            bought = self.bought_at.get((pid, ts), ())
            if pid in self.possessors and not bought:
                return
            self._remove(pid, ev[R.E_ITEM], where)
            self.destroyed[pid].append((ev[R.E_ITEM], ts))
            if ev[R.E_ITEM] in self.after and (not bought or (ev[R.E_ITEM] in bought and ev[R.E_ITEM] in self.at_once)):
                self.held[pid][self.after[ev[R.E_ITEM]]] += 1
        else:
            before, after = ev[R.E_UNDO_BEFORE], ev[R.E_UNDO_AFTER]
            if before and not after:
                at = next((i for i in range(len(self.buys[pid]) - 1, -1, -1) if self.buys[pid][i][0] == before), None)
                if at is None:
                    self.odd["undid a purchase never seen"] += 1
                    return
                _, bought = self.buys[pid].pop(at)
                self._remove(pid, before, where)
                back = [x for x in self.destroyed[pid] if x[1] == bought]
                self.destroyed[pid] = [x for x in self.destroyed[pid] if x[1] != bought]
                for item, _ in back:
                    self.held[pid][item] += 1
            elif after and not before:
                self.held[pid][after] += 1
            elif not before:
                self.odd["undo with no item"] += 1
            else:
                raise FormatError(f"{where}: ITEM_UNDO with {R.E_UNDO_BEFORE}={before} and {R.E_UNDO_AFTER}={after}")

    def gold(self, pid):
        return sum(self.values.get(i, 0) * n for i, n in self.held[pid].items())

    def items(self, pid):
        return collections.Counter({i: n for i, n in self.held[pid].items() if n > 0})


class Game:
    def __init__(self, match_id, mode, patch, y, champs, duration_min, minutes, states):
        self.match_id, self.mode, self.patch, self.y = match_id, mode, patch, y
        self.champs, self.duration_min = champs, duration_min
        self.minutes, self.states = minutes, states


def _int(v, where):
    if not isinstance(v, int) or isinstance(v, bool):
        raise FormatError(f"{where}: expected an integer, got {v!r}")
    return v


def _side(team_id, where):
    if team_id == R.BLUE:
        return 0
    if team_id == R.RED:
        return 1
    raise FormatError(f"{where}: team {team_id!r} is neither {R.BLUE} nor {R.RED}")


def read_match(match):
    """(match id, info, {participant id: side}, champions [[blue], [red]], skip reason or None)."""
    mid = need(need(match, R.M_METADATA, "match"), R.M_MATCH_ID, "match.metadata")
    info = need(match, R.M_INFO, mid)
    parts = need(info, R.M_PARTICIPANTS, f"{mid}.info")
    if len(parts) != 10:
        raise FormatError(f"{mid}: {len(parts)} participants")
    side, champs = {}, [[], []]
    for p in parts:
        where = f"{mid} participant"
        pid = _int(need(p, R.P_ID, where), where)
        side[pid] = _side(need(p, R.P_TEAM, where), where)
        champs[side[pid]].append(_int(need(p, R.P_CHAMP, where), where))
    if sorted(side) != list(range(1, 11)) or [len(c) for c in champs] != [5, 5]:
        raise FormatError(f"{mid}: participant ids {sorted(side)} and sides {[len(c) for c in champs]}")
    if any(need(p, f, f"{mid} participant") for p in parts for f in R.P_REMAKE_FLAGS):
        return mid, info, side, champs, "remake or early surrender"
    if _int(need(info, R.M_DURATION_S, mid), mid) < MIN_GAME_S:
        return mid, info, side, champs, "shorter than 5 min"
    return mid, info, side, champs, None


def blue_won(info, mid):
    teams = {_side(need(t, R.T_ID, f"{mid} team"), mid): need(t, R.T_WIN, f"{mid} team") for t in need(info, R.M_TEAMS, mid)}
    if sorted(teams) != [0, 1] or teams[0] == teams[1] or not all(isinstance(v, bool) for v in teams.values()):
        raise FormatError(f"{mid}: teams {teams}")
    return teams[0]


def read_events(frames, mid):
    """Every event of the timeline in time order, each checked against `riot_format`."""
    out = []
    for fi, f in enumerate(frames):
        for ei, ev in enumerate(need(f, R.F_EVENTS, f"{mid} frame {fi}")):
            where = f"{mid} frame {fi} event {ei}"
            kind = need(ev, R.E_TYPE, where)
            _int(need(ev, R.E_TIMESTAMP, where), where)
            if kind in R.USED_EVENTS:
                for field in R.USED_EVENTS[kind]:
                    need(ev, field, f"{where} ({kind})")
            elif kind not in R.IGNORED_EVENTS:
                raise FormatError(f"{where}: unknown event type {kind!r}; add it to riot_format.USED_EVENTS or IGNORED_EVENTS once checked")
            out.append((ev[R.E_TIMESTAMP], fi, ei, ev, where))
    out.sort(key=lambda e: e[:3])
    return out


def read_frames(timeline, mid):
    info = need(timeline, R.M_INFO, f"{mid} timeline")
    if need(info, R.TL_INTERVAL, f"{mid} timeline") != R.FRAME_MS:
        raise FormatError(f"{mid}: frame interval {info[R.TL_INTERVAL]}, expected {R.FRAME_MS}")
    frames = need(info, R.TL_FRAMES, f"{mid} timeline")
    want = {str(i) for i in range(1, 11)}
    for fi, f in enumerate(frames):
        where = f"{mid} frame {fi}"
        _int(need(f, R.F_TIMESTAMP, where), where)
        pf = need(f, R.F_PARTICIPANTS, where)
        if not isinstance(pf, dict) or set(pf) != want:
            raise FormatError(f"{where}: {R.F_PARTICIPANTS} keys {sorted(pf) if isinstance(pf, dict) else pf!r}")
        for k, v in pf.items():
            for field in R.PF_FIELDS:
                _int(need(v, field, f"{where} participant {k}"), f"{where} participant {k} {field}")
    if [f[R.F_TIMESTAMP] for f in frames] != sorted(f[R.F_TIMESTAMP] for f in frames):
        raise FormatError(f"{mid}: frames out of order")
    return frames


def final_items(participant):
    """What a player ends with: the seven slots and the role-bound one where the patch has it."""
    return collections.Counter(i for i in (*(participant[k] for k in R.P_ITEMS), participant.get(R.P_ROLE_ITEM, 0)) if i)


def adopt(orphans, items, final):
    """Purchases logged for nobody (each support's World Atlas at 0:00 comes with participant 0)
    go to the players who later lose that item, or end with it, without ever buying it."""
    first, given = {}, collections.defaultdict(set)
    for _, ev, _ in items:
        if ev[R.E_TYPE] != R.ITEM_UNDO:
            first.setdefault((ev[R.E_PARTICIPANT], ev[R.E_ITEM]), ev[R.E_TYPE])
    adopted = []
    for ts, ev, where in orphans:
        item = ev[R.E_ITEM]
        lost = lambda p: first.get((p, item)) in (R.ITEM_SOLD, R.ITEM_DESTROYED) or ((p, item) not in first and final[p][item] > 0)
        takers = [p for p in sorted(final) if p not in given[item] and lost(p)]
        if not takers:
            raise FormatError(f"{where}: item {item} bought by nobody and never lost or kept by a player who did not buy it")
        given[item].add(takers[0])
        adopted.append((ts, {**ev, R.E_PARTICIPANT: takers[0]}, where))
    return sorted(adopted + items, key=lambda x: x[0])


def parse_game(match, timeline, values):
    """(Game, None) or (None, why it is left out). Raises FormatError on an unknown shape."""
    mid, info, side, champs, skip = read_match(match)
    if skip:
        return None, skip
    mode = R.MAPS.get(need(info, R.M_MAP, mid))
    if mode is None:
        raise FormatError(f"{mid}: map {info[R.M_MAP]} is neither Summoner's Rift nor Howling Abyss")
    version = need(info, R.M_VERSION, mid)
    patch = ".".join(str(version).split(".")[:2])
    y = blue_won(info, mid)
    tl_id = need(need(timeline, R.M_METADATA, f"{mid} timeline"), R.M_MATCH_ID, f"{mid} timeline")
    if tl_id != mid:
        raise FormatError(f"{mid}: timeline is for {tl_id}")
    frames = read_frames(timeline, mid)
    events = read_events(frames, mid)

    def pid_ok(pid, where):
        if pid not in side:
            raise FormatError(f"{where}: participant {pid!r}")
        return pid

    kills, towers, inhibs, dragons, barons, grubs, souls, end = [], [], [], [], [], [], [], None
    items, orphans = [], []
    for ts, _, _, ev, where in events:
        kind, t = ev[R.E_TYPE], ts / 1000.0
        if kind == R.ITEM_PURCHASED and ev[R.E_PARTICIPANT] == R.NOBODY:
            orphans.append((ts, ev, where))
        elif kind in (R.ITEM_PURCHASED, R.ITEM_SOLD, R.ITEM_DESTROYED, R.ITEM_UNDO):
            pid_ok(ev[R.E_PARTICIPANT], where)
            items.append((ts, ev, where))
        elif kind == R.CHAMPION_KILL:
            killer, victim = _int(ev[R.E_KILLER], where), pid_ok(ev[R.E_VICTIM], where)
            if killer != 0:
                pid_ok(killer, where)
            kills.append((t, killer, victim))
        elif kind == R.BUILDING_KILL:
            b = ev[R.E_BUILDING]
            if b not in R.BUILDINGS:
                raise FormatError(f"{where}: building {b!r}")
            (towers if b == R.TOWER else inhibs).append((t, _side(ev[R.E_BUILDING_TEAM], where)))
        elif kind == R.ELITE_MONSTER_KILL:
            m = ev[R.E_MONSTER]
            if m in R.IGNORED_MONSTERS or (m == R.GRUB and ev[R.E_KILLER_TEAM] == R.NEUTRAL):
                continue
            taker = _side(ev[R.E_KILLER_TEAM], where)
            if m == R.DRAGON:
                sub = need(ev, R.E_MONSTER_SUB, where)
                if sub != R.ELDER and sub not in R.ELEMENTAL_DRAGONS:
                    raise FormatError(f"{where}: dragon {sub!r}")
                dragons.append((t, taker, sub == R.ELDER))
            elif m == R.BARON:
                barons.append((t, taker))
            elif m == R.GRUB:
                grubs.append((t, taker))
            else:
                raise FormatError(f"{where}: monster {m!r}")
        elif kind == R.DRAGON_SOUL_GIVEN:
            if ev[R.E_SOUL_TEAM] != R.NOBODY:
                souls.append(_side(ev[R.E_SOUL_TEAM], where))
        elif kind == R.GAME_END:
            if _side(ev[R.E_WINNER], where) != (0 if y else 1):
                raise FormatError(f"{where}: {R.GAME_END} winner {ev[R.E_WINNER]} disagrees with match teams")
            end = t
    for s in souls:
        if sum(1 for _, taker, elder in dragons if taker == s and not elder) < SOUL_DRAGONS:
            raise FormatError(f"{mid}: {R.DRAGON_SOUL_GIVEN} for side {s} with fewer than {SOUL_DRAGONS} dragons")
    items = adopt(orphans, items, {p[R.P_ID]: final_items(p) for p in info[R.M_PARTICIPANTS]})
    inv = Inventories(values, [(ev[R.E_PARTICIPANT], ts, ev[R.E_ITEM]) for ts, ev, _ in items if ev[R.E_TYPE] == R.ITEM_PURCHASED],
                      {p[R.P_ID] for p in info[R.M_PARTICIPANTS] if p[R.P_CHAMP] == R.POSSESSOR})

    respawn_back = INHIB_RESPAWN[mode]
    frame_s = [f[R.F_TIMESTAMP] / 1000.0 for f in frames]
    level_at = lambda pid, t: frames[max(i for i, ft in enumerate(frame_s) if ft <= t)][R.F_PARTICIPANTS][str(pid)][R.PF_LEVEL]
    kills = [(kt, killer, victim, kt + respawn(level_at(victim, kt), kt)) for kt, killer, victim in kills]
    rows = frames[:-1] if end is None else [f for f in frames if f[R.F_TIMESTAMP] / 1000.0 < end]
    minutes, states, done = [], [], 0
    for f in rows:
        t = f[R.F_TIMESTAMP] / 1000.0
        while done < len(items) and items[done][0] <= f[R.F_TIMESTAMP]:
            inv.apply(items[done][1], items[done][2])
            done += 1
        s = State()
        for pid in range(1, 11):
            i, pf = side[pid], f[R.F_PARTICIPANTS][str(pid)]
            s.item_gold[i] += inv.gold(pid)
            s.level[i] += pf[R.PF_LEVEL] / 5.0
            s.cs[i] += pf[R.PF_MINIONS] + pf[R.PF_JUNGLE]
        for kt, killer, victim, back in kills:
            if kt > t:
                continue
            if killer:
                s.kills[side[killer]] += 1
            left = back - t
            if left > 0:
                s.dead[side[victim]] += min(left, DEAD_CAP) / DEAD_CAP
        for bt, lost in towers:
            if bt <= t:
                s.towers[1 - lost] += 1
        for bt, lost in inhibs:
            if bt <= t < bt + respawn_back:
                s.inhibs[1 - lost] += 1
        for dt, taker, elder in dragons:
            if dt > t:
                continue
            if not elder:
                s.drakes[taker] += 1
            elif t < dt + ELDER_BUFF:
                s.elder = 1 if taker == 0 else -1
        for bt, taker in barons:
            if bt <= t < bt + BARON_BUFF:
                s.baron = 1 if taker == 0 else -1
        for gt, taker in grubs:
            if gt <= t:
                s.grubs[taker] += 1
        s.soul = int(s.drakes[0] >= SOUL_DRAGONS) - int(s.drakes[1] >= SOUL_DRAGONS)
        minutes.append(t / 60.0)
        states.append(s)
    while done < len(items):
        inv.apply(items[done][1], items[done][2])
        done += 1
    game = Game(mid, mode, patch, y, champs, info[R.M_DURATION_S] / 60.0, minutes, states)
    game.final_items = {pid: inv.items(pid) for pid in range(1, 11)}
    game.final_gold = {pid: inv.gold(pid) for pid in range(1, 11)}
    game.odd = inv.odd
    return game, None


def bucket(duration_min):
    return max(i for i, f in enumerate(LENGTH_FROM) if f <= duration_min)


def length_tables(games):
    """Champion → games and wins per `LENGTH_FROM` bucket."""
    t = collections.defaultdict(lambda: [[0, 0] for _ in LENGTH_FROM])
    for g in games:
        b = bucket(g.duration_min)
        for side in (0, 1):
            for c in g.champs[side]:
                t[c][b][0] += 1
                t[c][b][1] += int(g.y == (side == 0))
    return t


def outlook(game, minute, tables, own):
    """Σ blue (wr − 50) − Σ red (wr − 50). With `own`, the game's own result is taken out of the
    tables first (a training game must not see its outcome)."""
    total = 0.0
    b = bucket(game.duration_min)
    for side in (0, 1):
        for c in game.champs[side]:
            rows = tables.get(c)
            if rows is None:
                continue
            gw = [list(r) for r in rows]
            if own:
                gw[b][0] -= 1
                gw[b][1] -= int(game.y == (side == 0))
            wr = wr_from([(LENGTH_FROM[i], gw[i][0], gw[i][1]) for i in range(len(LENGTH_FROM))], minute)
            if wr is not None:
                total += (wr - 50.0) if side == 0 else (50.0 - wr)
    return total


def rows_of(games):
    """Arrays for fit.py, with the outlook from the training games' length tables."""
    train = [g for g in games if not is_test(g.match_id)]
    tables = dict(length_tables(train))
    game, minute, y, X, test, folds = [], [], [], [], [], []
    for gi, g in enumerate(games):
        held_out = is_test(g.match_id)
        for m, s in zip(g.minutes, g.states):
            s.outlook = outlook(g, m, tables, own=not held_out)
            game.append(gi)
            minute.append(m)
            y.append(float(g.y))
            X.append(s.features())
            test.append(held_out)
            folds.append(fold(g.match_id))
    import numpy as np
    return dict(game=np.array(game, np.int64), minute=np.array(minute), y=np.array(y), X=np.array(X).reshape(-1, len(KEYS)),
                test=np.array(test, bool), fold=np.array(folds, np.int8), match_ids=np.array([g.match_id for g in games]),
                keys=np.array(KEYS), train_games=np.int64(len(train)))


def save_rows(path, arrays, mode, patch, source):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    import numpy as np
    np.savez_compressed(path, mode=np.array(mode), patch=np.array(patch), source=np.array(source), **arrays)


def load_json(path):
    with (gzip.open(path, "rt", encoding="utf-8") if str(path).endswith(".gz") else open(path, encoding="utf-8")) as f:
        return json.load(f)


def raw_pairs(raw):
    for m in sorted(Path(raw).glob("*.match.json.gz")):
        tl = m.with_name(m.name.replace(".match.", ".timeline."))
        if tl.exists():
            yield m, tl


def ddragon_items(patch, cache_dir):
    import requests
    versions = requests.get(R.DDRAGON_VERSIONS, timeout=30).json()
    version = next((v for v in versions if v.startswith(patch + ".")), None)
    if version is None:
        sys.exit(f"Data Dragon has no version for patch {patch}")
    path = Path(cache_dir) / f"item-{version}.json"
    if not path.exists():
        r = requests.get(R.DDRAGON_ITEMS.format(version=version), timeout=60)
        r.raise_for_status()
        path.write_text(r.text, encoding="utf-8")
    return load_json(path)


def patch_of(match):
    return ".".join(str(match[R.M_INFO][R.M_VERSION]).split(".")[:2])


def version(patch):
    return tuple(int(x) for x in patch.split("."))


def check(raw, limit, values):
    """The plan T0b checks on up to `limit` crawled games; True when the agreement lines all
    hold and each team's item gold rebuilt from the events is close to that of its final items
    (`CHECK_TEAM_MEAN` on average, `CHECK_TEAM_WORST` at worst). Real timelines do not log every
    item change, so players are not all exact."""
    kinds, gaps, skipped = collections.Counter(), collections.Counter(), collections.Counter()
    agree, odd, nexus = collections.Counter(), collections.Counter(), collections.Counter()
    players = same_items = same_gold = 0
    worst, teams = [], []
    for n, (mp, tp) in enumerate(raw_pairs(raw)):
        if n >= limit:
            break
        match, timeline = load_json(mp), load_json(tp)
        game, why = parse_game(match, timeline, values)
        info, frames = match[R.M_INFO], timeline[R.M_INFO][R.TL_FRAMES]
        team = {p[R.P_ID]: p[R.P_TEAM] for p in info[R.M_PARTICIPANTS]}
        evs = [e for f in frames for e in f[R.F_EVENTS]]
        kinds.update(e[R.E_TYPE] for e in evs)
        ts = [f[R.F_TIMESTAMP] for f in frames]
        gaps.update(round((b - a) / 1000) for a, b in zip(ts, ts[1:]))
        if game is None:
            skipped[f"{why} ({info[R.M_DURATION_S]} s)"] += 1
            continue
        agree["duration within 60 s of the last frame", abs(ts[-1] / 1000 - info[R.M_DURATION_S]) <= 60] += 1
        loser = R.RED if game.y else R.BLUE
        for e in evs:
            killer = team.get(e.get(R.E_KILLER))
            if e[R.E_TYPE] == R.BUILDING_KILL and killer:
                agree["building teamId is not the killer's team", e[R.E_BUILDING_TEAM] != killer] += 1
            if e[R.E_TYPE] == R.BUILDING_KILL and e.get(R.E_TOWER_TYPE) == R.NEXUS_TURRET:
                nexus[e[R.E_BUILDING_TEAM] == loser] += 1
            if e[R.E_TYPE] == R.ELITE_MONSTER_KILL and killer:
                agree["monster killerTeamId is the killer's team", e[R.E_KILLER_TEAM] == killer] += 1
        odd.update(game.odd)
        sums = collections.defaultdict(lambda: [0, 0])
        for p in info[R.M_PARTICIPANTS]:
            pid = p[R.P_ID]
            final = final_items(p)
            want = sum(values.get(i, 0) * c for i, c in final.items())
            players += 1
            same_items += final == game.final_items[pid]
            same_gold += want == game.final_gold[pid]
            sums[p[R.P_TEAM]][0] += want
            sums[p[R.P_TEAM]][1] += game.final_gold[pid]
            if want != game.final_gold[pid]:
                worst.append((abs(want - game.final_gold[pid]), game.match_id, pid, dict(final - game.final_items[pid]), dict(game.final_items[pid] - final)))
        teams += [(got - want) / max(want, 1) for want, got in sums.values()]
    print(f"event types: {dict(kinds.most_common())}")
    print(f"seconds between frames: {dict(gaps.most_common(5))}; left out: {dict(skipped)}")
    for what in sorted({w for w, _ in agree}):
        print(f"{what}: {agree[what, True]} of {agree[what, True] + agree[what, False]}")
    print(f"nexus turrets on the losing side: {nexus[True]} of {nexus[True] + nexus[False]} (a winner can lose one; most must be the loser's)")
    print(f"final items rebuilt from the events: {same_items} of {players} players exact, {same_gold} same item gold")
    mean, top = (sum(abs(t) for t in teams) / len(teams), max(abs(t) for t in teams)) if teams else (1.0, 1.0)
    print(f"team item gold, rebuilt against final: off {100 * mean:.2f} % on average, {100 * top:.1f} % at worst, over {len(teams)} teams"
          f" (allowed {100 * CHECK_TEAM_MEAN:.0f} % and {100 * CHECK_TEAM_WORST:.0f} %)")
    print(f"item events that fit no inventory: {dict(odd.most_common())}")
    for w in sorted(worst, key=lambda w: w[:3], reverse=True)[:10]:
        print(f"  off by {w[0]} g: {w[1]} participant {w[2]} only in final {w[3]} only rebuilt {w[4]}")
    sides = nexus[True] >= CHECK_NEXUS * (nexus[True] + nexus[False])
    return players > 0 and mean <= CHECK_TEAM_MEAN and top <= CHECK_TEAM_WORST and sides and not any(agree[w, False] for w, _ in agree)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--raw", default=str(RAW))
    ap.add_argument("--out", default=str(ROWS))
    ap.add_argument("--mode", choices=sorted(MODE_KEYS), default="rift")
    ap.add_argument("--patch", help="keep games of this patch (default: the most common)")
    ap.add_argument("--items", help="Data Dragon item.json (default: fetched for the patch)")
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--limit", type=int, default=50)
    a = ap.parse_args(argv)
    pairs = list(raw_pairs(a.raw))
    if not pairs:
        sys.exit(f"no crawled games in {a.raw}")
    patches = collections.Counter(patch_of(load_json(m)) for m, _ in pairs[:200])
    patch = a.patch or patches.most_common(1)[0][0]
    values = item_values(load_json(a.items) if a.items else ddragon_items(patch, Path(a.raw).parent))
    if a.check:
        sys.exit(0 if check(a.raw, a.limit, values) else 1)
    games, skipped = [], collections.Counter()
    for n, (mp, tp) in enumerate(pairs):
        match = load_json(mp)
        if patch_of(match) != patch:
            skipped[f"patch {patch_of(match)}"] += 1
            continue
        game, why = parse_game(match, load_json(tp), values)
        if game is None:
            skipped[why] += 1
        elif game.mode != a.mode:
            skipped[f"mode {game.mode}"] += 1
        else:
            games.append(game)
        if (n + 1) % 500 == 0:
            print(f"{n + 1} of {len(pairs)} read", flush=True)
    arrays = rows_of(games)
    save_rows(a.out, arrays, a.mode, patch, "riot")
    print(f"{len(games)} games ({int(arrays['train_games'])} training), {len(arrays['y'])} rows to {a.out}; left out: {dict(skipped)}")


if __name__ == "__main__":
    main()
