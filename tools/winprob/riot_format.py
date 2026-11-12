"""Every assumption the win-probability tools make about Riot's API and its match-v5 JSON.

Checked against real replies on 2026-11-12 (patch 16.22, plan task T0b); README.md, "What the
real API showed", lists what differed from Riot's documentation and what is still unseen.
Code elsewhere reads these names only, so a correction is made here.
"""

PLATFORM_HOST = "https://{platform}.api.riotgames.com"
REGION_HOST = "https://{region}.api.riotgames.com"
KEY_HEADER = "X-Riot-Token"

LEAGUE_QUEUE = "RANKED_SOLO_5x5"
LEAGUE_ENTRIES = "/lol/league/v4/entries/{queue}/{tier}/{division}"
LEAGUE_APEX = {
    "CHALLENGER": "/lol/league/v4/challengerleagues/by-queue/{queue}",
    "GRANDMASTER": "/lol/league/v4/grandmasterleagues/by-queue/{queue}",
    "MASTER": "/lol/league/v4/masterleagues/by-queue/{queue}",
}
APEX_ENTRIES = "entries"
ENTRY_PUUID = "puuid"
LADDER_TIERS = ("DIAMOND", "EMERALD")
LADDER_DIVISIONS = ("I", "II", "III", "IV")

MATCH_IDS = "/lol/match/v5/matches/by-puuid/{puuid}/ids"
MATCH = "/lol/match/v5/matches/{match_id}"
TIMELINE = "/lol/match/v5/matches/{match_id}/timeline"
MATCH_IDS_PAGE = 100

DDRAGON_VERSIONS = "https://ddragon.leagueoflegends.com/api/versions.json"
DDRAGON_ITEMS = "https://ddragon.leagueoflegends.com/cdn/{version}/data/en_US/item.json"

M_METADATA = "metadata"
M_MATCH_ID = "matchId"
M_INFO = "info"
M_VERSION = "gameVersion"
M_QUEUE = "queueId"
M_MAP = "mapId"
M_DURATION_S = "gameDuration"
M_PARTICIPANTS = "participants"
M_TEAMS = "teams"
P_ID = "participantId"
P_TEAM = "teamId"
P_CHAMP = "championId"
# Seen 2026-11-12: Viego. Each possession sends ITEM_DESTROYED for all he holds, then for all the
# body held, and nothing when he has his own items back.
POSSESSOR = 234
P_REMAKE_FLAGS = ("gameEndedInEarlySurrender", "teamEarlySurrendered")
P_ITEMS = tuple(f"item{i}" for i in range(7))
# Seen 2026-11-12 (16.22): a slot of its own, a role quest reward or the boots of a player who has one.
P_ROLE_ITEM = "roleBoundItem"
T_ID = "teamId"
T_WIN = "win"
BLUE, RED = 100, 200
MAPS = {11: "rift", 12: "aram"}

TL_FRAMES = "frames"
TL_INTERVAL = "frameInterval"
FRAME_MS = 60000
F_TIMESTAMP = "timestamp"
F_PARTICIPANTS = "participantFrames"
F_EVENTS = "events"
PF_LEVEL = "level"
PF_MINIONS = "minionsKilled"
PF_JUNGLE = "jungleMinionsKilled"
PF_TOTAL_GOLD = "totalGold"
PF_CURRENT_GOLD = "currentGold"
PF_FIELDS = (PF_LEVEL, PF_MINIONS, PF_JUNGLE, PF_TOTAL_GOLD, PF_CURRENT_GOLD)

E_TYPE = "type"
E_TIMESTAMP = "timestamp"
E_PARTICIPANT = "participantId"
# Seen 2026-11-12: an id of 0 on ITEM_PURCHASED (each support's World Atlas at 0:00) and a team
# of 0 on DRAGON_SOUL_GIVEN (the soul's kind announced after the second dragon, nobody has it).
NOBODY = 0
E_ITEM = "itemId"
E_UNDO_BEFORE = "beforeId"
E_UNDO_AFTER = "afterId"
E_KILLER = "killerId"
E_VICTIM = "victimId"
E_BUILDING = "buildingType"
E_BUILDING_TEAM = "teamId"
E_MONSTER = "monsterType"
E_MONSTER_SUB = "monsterSubType"
E_KILLER_TEAM = "killerTeamId"
# Seen 2026-11-12: a killer team of 300 on a Rift Herald (88 of 1,122) and a Void Grub (8 of
# 3,609) that no side killed.
NEUTRAL = 300
OTHER_KEY = "Exception decrypting"
E_SOUL_TEAM = "teamId"
E_WINNER = "winningTeam"
E_TOWER_TYPE = "towerType"
NEXUS_TURRET = "NEXUS_TURRET"

ITEM_PURCHASED = "ITEM_PURCHASED"
ITEM_SOLD = "ITEM_SOLD"
ITEM_DESTROYED = "ITEM_DESTROYED"
ITEM_UNDO = "ITEM_UNDO"
CHAMPION_KILL = "CHAMPION_KILL"
BUILDING_KILL = "BUILDING_KILL"
ELITE_MONSTER_KILL = "ELITE_MONSTER_KILL"
DRAGON_SOUL_GIVEN = "DRAGON_SOUL_GIVEN"
GAME_END = "GAME_END"

USED_EVENTS = {
    ITEM_PURCHASED: (E_PARTICIPANT, E_ITEM),
    ITEM_SOLD: (E_PARTICIPANT, E_ITEM),
    ITEM_DESTROYED: (E_PARTICIPANT, E_ITEM),
    ITEM_UNDO: (E_PARTICIPANT, E_UNDO_BEFORE, E_UNDO_AFTER),
    CHAMPION_KILL: (E_KILLER, E_VICTIM),
    BUILDING_KILL: (E_BUILDING, E_BUILDING_TEAM),
    ELITE_MONSTER_KILL: (E_MONSTER, E_KILLER_TEAM),
    DRAGON_SOUL_GIVEN: (E_SOUL_TEAM,),
    GAME_END: (E_WINNER,),
}
IGNORED_EVENTS = frozenset({
    "PAUSE_START",
    "PAUSE_END",
    "SKILL_LEVEL_UP",
    "LEVEL_UP",
    "WARD_PLACED",
    "WARD_KILL",
    "TURRET_PLATE_DESTROYED",
    "CHAMPION_SPECIAL_KILL",
    "CHAMPION_TRANSFORM",
    "OBJECTIVE_BOUNTY_PRESTART",
    "OBJECTIVE_BOUNTY_FINISH",
    "FEAT_UPDATE",
})

TOWER = "TOWER_BUILDING"
INHIBITOR = "INHIBITOR_BUILDING"
BUILDINGS = frozenset({TOWER, INHIBITOR})
DRAGON = "DRAGON"
BARON = "BARON_NASHOR"
GRUB = "HORDE"
IGNORED_MONSTERS = frozenset({"RIFTHERALD", "ATAKHAN"})
ELDER = "ELDER_DRAGON"
ELEMENTAL_DRAGONS = frozenset({"FIRE_DRAGON", "WATER_DRAGON", "EARTH_DRAGON", "AIR_DRAGON", "HEXTECH_DRAGON", "CHEMTECH_DRAGON"})


class FormatError(Exception):
    """A reply that does not have the shape this module assumes."""


def need(obj, key, where):
    if not isinstance(obj, dict) or key not in obj:
        raise FormatError(f"{where}: no field {key!r} (have {sorted(obj) if isinstance(obj, dict) else type(obj).__name__})")
    return obj[key]
