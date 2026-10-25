"""Download the Set 18 unit meshes (OBJ per material) and colour textures from Community Dragon.

Usage: py -3.12 -I tools/tft-render/fetch.py [--out DIR] [--only api,api] [--refresh]

Writes <out>/<folder>/<skin>/<Mat>.obj, <out>/<folder>/<texture>.png and <out>/manifest.json.
Re-running skips files already on disk; directory listings are cached (listing.json) unless --refresh.
"""
import argparse, concurrent.futures as cf, datetime, json, os, re, sys, time
from pathlib import Path
from urllib.parse import unquote

import requests

GAME = "https://raw.communitydragon.org/latest/game/"
BASE = GAME + "assets/characters/"
SET = "TFTSet18"
REPO = Path(__file__).resolve().parents[2]
SET_JSON = REPO / "fixtures/tft/cdragon_set18.json"
OUT = Path(os.environ["LOCALAPPDATA"]) / "lol-coach/dev/render/assets"
JOBS = 8
# Second forms living in their own folder: extra mesh sources of the parent's api.
FORMS = {"tft18_gnar": ["tft18_gnarbig"], "tft18_elise": ["tft18_elisespider"], "tft18_nidalee": ["tft18_nidaleecougar"]}
# Summons with their own folder (own api = folder) → the unit that makes them. Whether each shows an HP bar
# in game is not verified; the evaluation decides which stay.
SUMMONS = {"tft18_azirsoldier": "DA_18_Azir", "tft18_yorickghoul": "DA_18_Yorick", "tft18_maokaisapling": "DA_18_Maokai",
           "tft18_zyragraspingplant": "DA_18_Zyra", "tft18_zyrathornplant": "DA_18_Zyra", "tft18_zyrathornspitter": "DA_18_Zyra",
           "tft18_zyravinelasher": "DA_18_Zyra", "tft18_raptormini": "DA_CrimsonRaptor18", "tft18_krugmini": "DA_Krug18",
           "tft18_krugminimini": "DA_Krug18"}
# Data folder (game/data/characters/<x>/skins/skin0.bin.json) when it isn't the unit's characterName / the folder.
DATA_NAME = {"tft18_yorickghoul": "tft18_yorickmeleeghoul"}
SHARED = ("assets/shared/",)
# Texture when the skin bin names none. Zyra's mesh is LoL Zyra's (same 4624 verts and UVs); the sapling's is a guess.
TEX_FALLBACK = {"tft18_zyra": "assets/characters/zyra/zyra_base_tx_cm.tex",
                "tft18_maokaisapling": "assets/characters/maokai/skins/base/maokai_base_2014_tx_cm.tex"}

S = requests.Session()
S.headers["User-Agent"] = "lol-coach tft-render fetch"


def get(url, tries=4):
    for i in range(tries):
        try:
            r = S.get(url, timeout=60)
            if r.status_code == 404:
                return None
            r.raise_for_status()
            return r
        except requests.RequestException:
            if i == tries - 1:
                raise
            time.sleep(1 + 2 * i)


def listing(url):
    """(dirs, files) of a Community Dragon directory page; [] when it doesn't exist."""
    r = get(url)
    if r is None:
        return [], []
    names = [h for h in re.findall(r'href="([^"]+)" title=', r.text) if not h.startswith(("http", "?", "/", ".."))]
    return sorted(n[:-1] for n in names if n.endswith("/")), sorted(n for n in names if not n.endswith("/"))


def folder_listing(folder, cache, refresh):
    """{"skins": {skin_dir: [obj..]}, "textures": [png..]} for skins/base of a folder, cached per folder."""
    p = cache / folder / "listing.json"
    if p.exists() and not refresh:
        return json.loads(p.read_text())
    base = f"{BASE}{folder}/skins/base/"
    dirs, files = listing(base)
    out = {"skins": {}, "textures": [f for f in files if f.lower().endswith(".png")], "skn": [f for f in files if f.endswith(".skn")]}
    for d in dirs:
        if d == "images":
            continue
        out["skins"][d] = [f for f in listing(base + d + "/")[1] if f.lower().endswith(".obj")]
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps(out, indent=1))
    return out


def download(url, dest):
    if dest.exists() and dest.stat().st_size > 0:
        return False
    r = get(url)
    if r is None:
        raise FileNotFoundError(url)
    dest.parent.mkdir(parents=True, exist_ok=True)
    tmp = dest.with_suffix(dest.suffix + ".part")
    tmp.write_bytes(r.content)
    tmp.replace(dest)
    return True


def obj_stats(path):
    """(vertex count, [min xyz, max xyz]) of an OBJ, in its own Y-up units."""
    lo, hi, n = [1e30] * 3, [-1e30] * 3, 0
    with open(path, "rb") as f:
        for line in f:
            if line.startswith(b"v "):
                v = [float(x) for x in line.split()[1:4]]
                lo, hi, n = [min(a, b) for a, b in zip(lo, v)], [max(a, b) for a, b in zip(hi, v)], n + 1
    return n, [[round(x, 1) for x in lo], [round(x, 1) for x in hi]] if n else None


def diffuse(mat):
    """Colour texture path of a StaticMaterialDef: a `*tx_cm*` sampler named Main/Diffuse, else any `*tx_cm*`, else Diffuse."""
    sv = [(x.get("TextureName") or x.get("textureName") or "", x.get("texturePath") or x.get("texture") or "") for x in mat.get("samplerValues", [])]
    sv = [(n.lower(), t.lower()) for n, t in sv if t and not t.lower().startswith(SHARED)]
    return (next((t for n, t in sv if "tx_cm" in t and ("main" in n or "diffuse" in n)), None) or next((t for n, t in sv if "tx_cm" in t), None)
            or next((t for n, t in sv if "diffuse" in n), None))


def skin_info(d):
    """{texture, hide, scale, submesh_textures} from a skin0.bin.json (game paths, lowercase .tex)."""
    smp, mats = None, {}
    for k, v in d.items():
        if isinstance(v, dict):
            if v.get("__type") == "StaticMaterialDef":
                mats[k] = diffuse(v)
            if smp is None and "skinMeshProperties" in v:
                smp = v["skinMeshProperties"]
    if smp is None:
        return {}
    sub = {o["submesh"]: (o.get("texture") or "").lower() or mats.get(o.get("Material")) for o in smp.get("materialOverride", []) if o.get("submesh")}
    # The base material's sampler wins over `texture` (Sivir's `texture` is her recall scenery).
    tex = mats.get(smp.get("Material")) or (smp.get("texture") or "").lower()
    m = re.match(r"assets/characters/([^/]+)/", (smp.get("simpleSkin") or "").lower())
    return {"texture": tex if tex and not tex.startswith(SHARED) else None, "hide": [h for h in re.split(r"[\s,]+", smp.get("initialSubmeshToHide") or "") if h],
            "mesh_folder": m.group(1) if m else None,
            "scale": smp.get("skinScale", 1.0), "submesh_textures": {k: v for k, v in sub.items() if v and not v.startswith(SHARED)}}


def tex_dest(folder, path):
    """Local path (relative to the assets dir) for a game texture path."""
    name = Path(path).with_suffix(".png").name
    own = f"assets/characters/{folder}/skins/base/"
    return f"{folder}/{name}" if path.startswith(own) and "/" not in path[len(own):] else f"{folder}/ext/{name}"


def set_units():
    d = json.loads(SET_JSON.read_text(encoding="utf-8"))
    s = next(x for x in d["setData"] if x["mutator"] == SET)
    units, skipped = [], []
    for c in s["champions"]:
        if not c.get("traits") or not 1 <= c["cost"] <= 5:
            skipped.append({"api": c["apiName"], "why": "no traits or cost outside 1-5 (PvE, dummy, key, chest)"})
            continue
        m = re.match(r"assets/characters/([^/]+)/", (c.get("tileIcon") or c.get("squareIcon") or "").lower())
        folder = m.group(1) if m else "tft18_" + re.sub(r"[^a-z]", "", c["characterName"].lower().replace("da_", "").replace("18", ""))
        units.append({"api": c["apiName"], "name": c["name"], "cost": c["cost"], "traits": c["traits"], "kind": "unit", "folder": folder,
                      "data": c["characterName"].lower()})
    return units, skipped


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", type=Path, default=OUT)
    ap.add_argument("--only", help="comma-separated api names (default: all)")
    ap.add_argument("--refresh", action="store_true", help="re-read directory listings")
    a = ap.parse_args()
    out = a.out
    units, skipped = set_units()
    by_api = {u["api"]: u for u in units}
    for folder, parent in SUMMONS.items():
        units.append({"api": folder, "name": folder.removeprefix("tft18_"), "cost": by_api[parent]["cost"], "traits": [], "kind": "summon",
                      "summon_of": parent, "folder": folder, "data": DATA_NAME.get(folder, folder)})
    if a.only:
        keep = set(a.only.split(","))
        units = [u for u in units if u["api"] in keep]

    all_dirs = [d for d in listing(BASE)[0] if d.startswith("tft18_")]
    known = {u["folder"] for u in units} | {f for fs in FORMS.values() for f in fs} | set(SUMMONS)
    folders = sorted({u["folder"] for u in units} | {f for u in units for f in FORMS.get(u["folder"], [])})
    print(f"{len(units)} units, {len(folders)} folders; listing...", flush=True)
    with cf.ThreadPoolExecutor(JOBS) as ex:
        lists = dict(zip(folders, ex.map(lambda f: folder_listing(f, out, a.refresh), folders)))
    data_of = {u["folder"]: u["data"] for u in units} | {f: DATA_NAME.get(f, f) for f in folders if f not in {u["folder"] for u in units}}

    def bin_info(f):
        p = out / f / "skin0.bin.json"
        if not p.exists() or a.refresh:
            r = get(f"{GAME}data/characters/{data_of[f]}/skins/skin0.bin.json")
            p.write_text(r.text if r is not None else "{}", encoding="utf-8")
        i = skin_info(json.loads(p.read_text(encoding="utf-8")))
        if not i.get("texture") and f in TEX_FALLBACK:
            i |= {"texture": TEX_FALLBACK[f], "texture_guess": True}
        return i

    with cf.ThreadPoolExecutor(JOBS) as ex:
        infos = dict(zip(folders, ex.map(bin_info, folders)))

    jobs = []
    for f, l in lists.items():
        for skin, objs in l["skins"].items():
            jobs += [(f"{BASE}{f}/skins/base/{skin}/{o}", out / f / skin / o) for o in objs]
        jobs += [(f"{BASE}{f}/skins/base/{t}", out / f / t) for t in l["textures"]]
        i = infos[f]
        for t in {i.get("texture"), *i.get("submesh_textures", {}).values()} - {None}:
            dest = tex_dest(f, t)
            if "/ext/" in dest:
                jobs.append((GAME + str(Path(t).with_suffix(".png")).replace("\\", "/"), out / dest))
    t0, fails = time.time(), []
    with cf.ThreadPoolExecutor(JOBS) as ex:
        futs = {ex.submit(download, u, p): u for u, p in jobs}
        got = 0
        for fu in cf.as_completed(futs):
            try:
                got += fu.result()
            except Exception as e:
                fails.append(f"{futs[fu]}: {e}")
    print(f"{len(jobs)} files, {got} downloaded in {time.time() - t0:.0f}s, {len(fails)} failed", flush=True)

    def source(f):
        l = lists[f]
        i = infos[f]
        mf = f if l["skins"] or i.get("mesh_folder") not in lists else i["mesh_folder"]  # krugminimini uses krugmini's mesh
        objs = [{"name": unquote(Path(o).stem), "path": f"{mf}/{skin}/{o}", **dict(zip(("verts", "bbox"), obj_stats(out / mf / skin / o)))}
                for skin, os_ in lists[mf]["skins"].items() for o in os_ if (out / mf / skin / o).exists()]
        have = lambda t: t if t and (out / t).exists() else None
        return {"folder": f, "data": data_of[f], "objs": objs, "textures": [f"{f}/{t}" for t in l["textures"] if (out / f / t).exists()],
                "texture": have(i.get("texture") and tex_dest(f, i["texture"])), "hide": i.get("hide", []), "scale": i.get("scale", 1.0), "texture_guess": i.get("texture_guess", False),
                "submesh_textures": {k: have(tex_dest(f, v)) for k, v in i.get("submesh_textures", {}).items() if have(tex_dest(f, v))}}

    manifest_units, missing, first = [], [], {}
    for u in units:
        f = u["folder"]
        if f not in all_dirs:
            missing.append({"api": u["api"], "folder": f, "why": "no such Community Dragon folder"})
            continue
        if f in first and u["kind"] == "unit":
            manifest_units.append({**u, "alias_of": first[f], "sources": []})
            continue
        first.setdefault(f, u["api"])
        srcs = [source(f)] + [source(x) for x in FORMS.get(f, [])]
        why = [w for w, bad in (("no OBJ meshes", not srcs[0]["objs"]), ("no texture", not (srcs[0]["textures"] or srcs[0]["texture"]))) if bad]
        if why:
            missing.append({"api": u["api"], "folder": f, "why": ", ".join(why)})
        manifest_units.append({**u, "sources": srcs})
    manifest = {"generated": datetime.datetime.now().isoformat(timespec="seconds"), "base_url": BASE, "set": SET,
                "units": manifest_units, "missing": missing, "skipped": skipped, "failed_downloads": fails,
                "not_in_set": sorted(set(all_dirs) - known)}
    (out / "manifest.json").write_text(json.dumps(manifest, indent=1))
    ok = sum(1 for u in manifest_units if u["sources"] and not any(m["api"] == u["api"] for m in missing))
    print(f"manifest: {len(manifest_units)} entries ({ok} renderable, {sum('alias_of' in u for u in manifest_units)} aliases), "
          f"{len(missing)} missing, {len(manifest['not_in_set'])} tft18 folders not in the set -> {out / 'manifest.json'}")
    for m in missing:
        print(f"  missing {m['api']} ({m['folder']}): {m['why']}")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
