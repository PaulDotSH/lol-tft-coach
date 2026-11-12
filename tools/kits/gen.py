import argparse
import json
import re
import statistics
import sys
import urllib.request
from pathlib import Path

KEYS = "QWER"
KINDS = {"physicalDamage": "physical", "magicDamage": "magic", "trueDamage": "true"}
RATIOS = ["ap", "ad", "bonus_ad", "hp", "bonus_hp", "armor", "bonus_armor", "mr", "bonus_mr"]
STATS = {0: "ap", 1: "armor", 2: "ad", 6: "mr", 12: "hp"}
TAG = re.compile(r"<(physicalDamage|magicDamage|trueDamage)>(.*?)</\1>", re.S)
SLOT = re.compile(r"\{\{\s*([A-Za-z0-9_{}]+)\s*(?:\*\s*([0-9.]+))?\s*\}\}")
AGENT = {"User-Agent": "kits-gen/1"}
MIN_COOLDOWN = 2.0


class Skip(Exception):
    pass


def fetch(url, path):
    if not path.exists():
        req = urllib.request.Request(url, headers=AGENT)
        with urllib.request.urlopen(req, timeout=60) as r:
            path.write_bytes(r.read())
    return json.loads(path.read_text(encoding="utf-8"))


class Linear:
    def __init__(self, ranks):
        self.ranks = ranks
        self.terms = {"base": [0.0] * ranks}

    @staticmethod
    def const(ranks, values):
        out = Linear(ranks)
        out.terms["base"] = list(values)
        return out

    def is_const(self):
        return all(k == "base" for k, v in self.terms.items() if any(v))

    def add(self, other):
        for k, v in other.terms.items():
            cur = self.terms.setdefault(k, [0.0] * self.ranks)
            self.terms[k] = [a + b for a, b in zip(cur, v)]
        return self

    def scale(self, by):
        for k in self.terms:
            self.terms[k] = [a * b for a, b in zip(self.terms[k], by)]
        return self

    def stat(self, stat, formula, coef):
        name = STATS.get(stat)
        if name is None:
            raise Skip(f"stat {stat}")
        if formula == 2:
            if name == "ap":
                raise Skip("bonus ap")
            terms = {f"bonus_{name}": coef}
        elif formula == 1:
            if name != "ad":
                raise Skip(f"base {name}")
            terms = {"ad": coef, "bonus_ad": [-c for c in coef]}
        elif formula in (0, None):
            terms = {name: coef}
        else:
            raise Skip(f"formula {formula}")
        for k, v in terms.items():
            cur = self.terms.setdefault(k, [0.0] * self.ranks)
            self.terms[k] = [a + b for a, b in zip(cur, v)]
        return self


class Spell:
    def __init__(self, spell, ranks):
        self.spell = spell
        self.ranks = ranks
        self.calcs = {k.lower(): v for k, v in (spell.get("mSpellCalculations") or {}).items()}
        self.values = {d["name"].lower(): d.get("values", []) for d in spell.get("DataValues", []) if "name" in d}

    def value(self, name):
        v = self.values.get(name.lower())
        if not v or len(v) < self.ranks + 1:
            raise Skip(f"value {name}")
        return [float(x) for x in v[1 : self.ranks + 1]]

    def const(self, part):
        lin = self.part(part)
        if not lin.is_const():
            raise Skip("non-constant factor")
        return lin.terms["base"]

    def part(self, p):
        t = p.get("__type")
        r = self.ranks
        if t == "NamedDataValueCalculationPart":
            return Linear.const(r, self.value(p["mDataValue"]))
        if t == "NumberCalculationPart":
            return Linear.const(r, [float(p.get("mNumber", 0.0))] * r)
        if t == "StatByCoefficientCalculationPart":
            return Linear(r).stat(p.get("mStat", 0), p.get("mStatFormula"), [float(p.get("mCoefficient", 0.0))] * r)
        if t == "StatByNamedDataValueCalculationPart":
            return Linear(r).stat(p.get("mStat", 0), p.get("mStatFormula"), self.value(p["mDataValue"]))
        if t == "StatBySubPartCalculationPart":
            return Linear(r).stat(p.get("mStat", 0), p.get("mStatFormula"), self.const(p["mSubpart"]))
        if t == "ProductOfSubPartsCalculationPart":
            a, b = self.part(p["mPart1"]), self.part(p["mPart2"])
            if a.is_const():
                return b.scale(a.terms["base"])
            if b.is_const():
                return a.scale(b.terms["base"])
            raise Skip("product of two variables")
        if t == "SumOfSubPartsCalculationPart":
            out = Linear(r)
            for s in p.get("mSubparts", []):
                out.add(self.part(s))
            return out
        if t == "EffectValueCalculationPart":
            effects = self.spell.get("mEffectAmount", [])
            i = p.get("mEffectIndex", 1) - 1
            if not 0 <= i < len(effects):
                raise Skip("effect index")
            v = effects[i].get("value", [])
            if len(v) < r + 1:
                raise Skip("effect values")
            return Linear.const(r, [float(x) for x in v[1 : r + 1]])
        if t == "ByCharLevelInterpolationCalculationPart":
            mid = (float(p.get("mStartValue", 0.0)) + float(p.get("mEndValue", 0.0))) / 2
            return Linear.const(r, [mid] * r)
        raise Skip(f"part {t}")

    def calc(self, name, depth=0):
        c = self.calcs.get(name.lower())
        if c is None:
            return Linear.const(self.ranks, self.value(name))
        if depth > 4:
            raise Skip("calc depth")
        if c.get("mDisplayAsPercent"):
            raise Skip("percent")
        t = c.get("__type")
        if t == "GameCalculationModified":
            out = self.calc(c["mModifiedGameCalculation"], depth + 1)
        elif t == "GameCalculation":
            out = Linear(self.ranks)
            for p in c.get("mFormulaParts", []):
                out.add(self.part(p))
        else:
            raise Skip(f"calc {t}")
        if "mMultiplier" in c:
            out.scale(self.const(c["mMultiplier"]))
        return out


def damage_slots(tooltip):
    seen, out = set(), []
    for kind, inner in TAG.findall(tooltip):
        m = SLOT.search(inner)
        if not m or KINDS[kind] in seen:
            continue
        seen.add(KINDS[kind])
        out.append((KINDS[kind], m.group(1), float(m.group(2) or 1)))
    return out


def num(x):
    x = round(x, 3)
    return str(int(x)) if x == int(x) else repr(x)


def arr(v):
    return "[" + ", ".join(num(x) for x in v) + "]"


def per_rank(v):
    return num(v[0]) if all(abs(x - v[0]) < 1e-6 for x in v) else arr(v)


def parse_spell(key, bin_spell, dd_spell, skipped):
    ranks = dd_spell["maxrank"]
    sp = Spell(bin_spell, ranks)
    out = []
    if max(dd_spell["cooldown"][:ranks] or [0]) < MIN_COOLDOWN:
        skipped.append(f"{key}: toggle or stance (cooldown under {MIN_COOLDOWN} s)")
        return out
    for kind, name, mult in damage_slots(dd_spell.get("tooltip", "")):
        try:
            if mult >= 10:
                raise Skip("display multiplier")
            lin = sp.calc(name)
        except Skip as e:
            skipped.append(f"{key} {name}: {e}")
            continue
        terms = {k: v for k, v in lin.terms.items() if any(abs(x) > 1e-6 for x in v)}
        if not terms:
            continue
        s = {"key": key, "kind": kind, "base": lin.terms["base"], "cooldown": [float(c) for c in dd_spell["cooldown"][:ranks]]}
        s.update({k: terms[k] for k in RATIOS if k in terms})
        if mult != 1:
            s["hits"] = mult
        out.append(s)
    return out


def root_record(binj, champ_id):
    want = f"characters/{champ_id.lower()}/characterrecords/root"
    for k, v in binj.items():
        if k.lower() == want:
            return v
    raise Skip("no root record")


def base_ad(root, dd):
    def mod(name):
        v = root.get(name)
        return float(v.get("baseValue", 0.0)) if isinstance(v, dict) else None

    ad, per = mod("baseDamageModifiable"), mod("damagePerLevelModifiable")
    return (ad if ad is not None else dd["stats"]["attackdamage"], per if per is not None else dd["stats"]["attackdamageperlevel"])


def parse_champion(binj, dd):
    skipped = []
    root = root_record(binj, dd["id"])
    spells = []
    for i, key in enumerate(KEYS):
        paths = root.get("spells", [])
        rec = binj.get(paths[i]) if i < len(paths) else None
        if not rec or "mSpell" not in rec or i >= len(dd["spells"]):
            skipped.append(f"{key}: no spell record")
            continue
        spells += parse_spell(key, rec["mSpell"], dd["spells"][i], skipped)
    ad, per = base_ad(root, dd)
    return {"base_ad": ad, "ad_per_level": per, "spells": spells, "skipped": skipped}


def class_profiles(champs, dd_all):
    out = {}
    by_class = {}
    for cid, kit in champs.items():
        tags = dd_all[cid].get("tags") or []
        if tags and kit["spells"]:
            by_class.setdefault(tags[0], []).append(kit)
    for cls, kits in sorted(by_class.items()):
        spells = []
        for key in KEYS:
            got = [s for k in kits for s in k["spells"] if s["key"] == key]
            if not got:
                continue
            n = statistics.mode(len(s["base"]) for s in got)
            got = [s for s in got if len(s["base"]) == n]
            kind = statistics.mode(s["kind"] for s in got)
            med = lambda f: [statistics.median(f(s)[i] for s in got) for i in range(n)]
            s = {"key": key, "kind": kind, "base": med(lambda s: s["base"]), "cooldown": med(lambda s: s["cooldown"])}
            for r in RATIOS:
                v = med(lambda s: s.get(r, [0.0] * n))
                if any(abs(x) > 1e-6 for x in v):
                    s[r] = v
            spells.append(s)
        out["_" + cls] = {"base_ad": statistics.median(k["base_ad"] for k in kits), "ad_per_level": statistics.median(k["ad_per_level"] for k in kits), "spells": spells}
    return out


def emit(name, kit, lines):
    lines.append(f"[{name}]")
    lines.append(f"base_ad = {num(kit['base_ad'])}")
    lines.append(f"ad_per_level = {num(kit['ad_per_level'])}")
    if not kit["spells"]:
        lines.append("# rough: no damage parsed, class fallback")
    for s in kit["spells"]:
        lines.append(f"[[{name}.spell]]")
        lines.append(f'key = "{s["key"]}"')
        lines.append(f'kind = "{s["kind"]}"')
        lines.append(f"base = {arr(s['base'])}")
        for r in RATIOS:
            if r in s:
                lines.append(f"{r} = {per_rank(s[r])}")
        if "hits" in s:
            lines.append(f"hits = {num(s['hits'])}")
        lines.append(f"cooldown = {arr(s['cooldown'])}")
    lines.append("")


def main():
    ap = argparse.ArgumentParser(description="Draft kits.toml from Community Dragon champion bins and Data Dragon tooltips.")
    ap.add_argument("--patch", required=True, help="Community Dragon patch, e.g. 16.21")
    ap.add_argument("--dd", required=True, help="Data Dragon version, e.g. 16.21.1")
    ap.add_argument("--cache", required=True, type=Path, help="download folder")
    ap.add_argument("--out", type=Path, default=Path("crates/coach/kits.toml"))
    ap.add_argument("--only", nargs="*", help="Data Dragon ids to process")
    a = ap.parse_args()
    a.cache.mkdir(parents=True, exist_ok=True)
    dd_all = fetch(f"https://ddragon.leagueoflegends.com/cdn/{a.dd}/data/en_US/championFull.json", a.cache / f"championFull-{a.dd}.json")["data"]
    champs, rough = {}, []
    for cid in sorted(dd_all):
        if a.only and cid not in a.only:
            continue
        low = cid.lower()
        try:
            binj = fetch(f"https://raw.communitydragon.org/{a.patch}/game/data/characters/{low}/{low}.bin.json", a.cache / f"{low}-{a.patch}.bin.json")
            kit = parse_champion(binj, dd_all[cid])
        except Exception as e:
            print(f"{cid}: {e}", file=sys.stderr)
            dd = dd_all[cid]
            kit = {"base_ad": dd["stats"]["attackdamage"], "ad_per_level": dd["stats"]["attackdamageperlevel"], "spells": [], "skipped": [str(e)]}
        champs[cid] = kit
        if not kit["spells"]:
            rough.append(cid)
        for s in kit["skipped"]:
            print(f"{cid} skipped {s}", file=sys.stderr)
    lines = [
        f"# GENERATED by tools/kits/gen.py from Community Dragon {a.patch} champion bins and Data Dragon {a.dd} tooltips.",
        "# UNCHECKED: no hand pass yet. Damage per cast at each rank, ratios on total AP / total AD / bonus AD / max HP / resists.",
        "# One entry per damage type in the tooltip (the first value of each type). Champions without entries use their class",
        "# profile (_Mage, _Marksman, ...: medians of the parsed champions) and are rough.",
        f"# {len(champs) - len(rough)} champions parsed, {len(rough)} rough: {', '.join(rough) or 'none'}.",
        "",
    ]
    for name, kit in class_profiles(champs, dd_all).items():
        emit(name, kit, lines)
    for cid, kit in champs.items():
        emit(cid, kit, lines)
    a.out.write_text("\n".join(lines), encoding="utf-8", newline="\n")
    print(f"{len(champs)} champions, {len(champs) - len(rough)} parsed, {len(rough)} rough", file=sys.stderr)


if __name__ == "__main__":
    main()
