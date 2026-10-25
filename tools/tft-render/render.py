"""Render Set 18 units (bind pose) from Community Dragon OBJs with Blender, as the fixed TFT camera sees them.

Run inside Blender (headless):
  blender.exe -b -P tools/tft-render/render.py -- [--units A,B | --units all] [--params P.json] [--set key=value ...]
                                                  [--manifest M] [--out DIR] [--force]
List the variants without Blender:
  py -3.12 -I tools/tft-render/render.py --list [--units ...]

Output per unit: <out>/<api>/<variant>_<yaw>_<size>_<light>.png (RGBA, transparent background) and <out>/<api>/meta.json.
Pixel coordinates in meta are from the image's top-left. A unit is skipped when its meta.json has the same params hash and
variants (meshes per variant) and all its files exist (resumable); --force re-renders.
"""
import argparse, hashlib, json, os, re, subprocess, sys, time
from pathlib import Path

HERE = Path(__file__).resolve().parent
DEV = Path(os.environ.get("LOCALAPPDATA", ".")) / "lol-coach/dev/render"
GAMES = ("League of Legends.exe", "TFTClient-Win64-Shipping.exe")
GAME_POLL_S = 180
# Hidden meshes that are never a look of the unit (recall props, VFX cards, smears).
NOT_FORM = re.compile(r"recall|return|smear|vfx", re.I)
# Extra model turn; 0 because the OBJ importer (forward -Z, up Y) already turns the model (OBJ +Z) to the camera on -Y.
YAW0 = 0.0


def load_params(path, sets):
    p = json.loads(Path(path).read_text())
    for kv in sets or []:
        k, v = kv.split("=", 1)
        p[k] = json.loads(v)
    return p


def params_hash(p):
    return hashlib.sha1(json.dumps(p, sort_keys=True).encode()).hexdigest()[:12]


def variants(unit, p):
    """[(variant id, source index, [obj names])] for a manifest unit.

    Per mesh source (folder): `base` = meshes shown by default (not in the skin's initialSubmeshToHide or hidden by an Idle
    clip, unless an Idle clip shows them; never VFX/recall).
    Extra folders (Gnar big, Elise spider, Nidalee cougar) are their own base, named by folder. A hidden mesh with at least
    `form_frac` of the largest shown mesh's vertices is a possible form: `with-<mesh>` (added to base) and, when it is at
    least `form_height` of the shown meshes' height, also `only-<mesh>` (alone, a whole-body form like Ahri's fox), as
    `form_modes` allows. The evaluation decides which variants stay.
    """
    out = []
    for si, s in enumerate(unit["sources"]):
        # Idle clips' submesh events win over the skin's default list (Ahri's idle drops her single tail, Fiddlesticks' adds
        # his lantern and demon arm).
        idle_hide, idle_show = {h.lower() for h in s.get("idle_hide", [])}, {h.lower() for h in s.get("idle_show", [])}
        hide = ({h.lower() for h in s["hide"]} | idle_hide) - idle_show
        objs = s["objs"]
        shown = [o for o in objs if o["name"].lower() not in hide and not NOT_FORM.search(o["name"])]
        if not shown:
            continue
        stem = "base" if si == 0 else s["folder"].removeprefix("tft18_")
        out.append((stem, si, [o["name"] for o in shown]))
        big = max(o["verts"] for o in shown)
        height = lambda os_: max(o["bbox"][1][1] for o in os_) - min(o["bbox"][0][1] for o in os_)
        tall = height(shown)
        for o in objs:
            if o["name"].lower() in hide and not NOT_FORM.search(o["name"]) and o["verts"] >= p["form_frac"] * big:
                tag = re.sub(r"[^a-z0-9]+", "-", o["name"].lower()).strip("-")
                pre = "" if si == 0 else stem + "-"
                if "alone" in p["form_modes"] and height([o]) >= p["form_height"] * tall:
                    out.append((f"{pre}only-{tag}", si, [o["name"]]))
                if "add" in p["form_modes"]:
                    out.append((f"{pre}with-{tag}", si, [x["name"] for x in shown] + [o["name"]]))
    return out


def pick_units(manifest, spec):
    units = [u for u in manifest["units"] if u["sources"] and "alias_of" not in u]
    if spec in (None, "all"):
        return units
    want = spec.split(",")
    by = {u["api"].lower(): u for u in units}
    missing = [w for w in want if w.lower() not in by]
    if missing:
        sys.exit(f"unknown or alias units: {missing}")
    return [by[w.lower()] for w in want]


def game_running():
    try:
        out = subprocess.run(["tasklist", "/FO", "CSV", "/NH"], capture_output=True, text=True, timeout=30).stdout
    except Exception:
        return False
    return any(g.lower() in out.lower() for g in GAMES)


def wait_for_no_game():
    while game_running():
        print(f"game running; waiting {GAME_POLL_S}s", flush=True)
        time.sleep(GAME_POLL_S)


def file_name(var, yaw, size, light):
    return f"{var}_{yaw:03d}_{size:.2f}_{light}.png"


# ---------------------------------------------------------------- Blender side

def blender_main(a, p, manifest, units):
    import bpy, numpy as np
    from mathutils import Matrix

    assets = Path(a.manifest).parent
    sc = bpy.context.scene
    res = p["res"]

    def setup_scene():
        bpy.ops.wm.read_homefile(use_empty=True)
        sc_ = bpy.context.scene
        for eng in ("BLENDER_EEVEE", "BLENDER_EEVEE_NEXT"):
            try:
                sc_.render.engine = eng
                break
            except TypeError:
                continue
        sc_.eevee.taa_render_samples = p["samples"]
        sc_.render.resolution_x = sc_.render.resolution_y = res
        sc_.render.resolution_percentage = 100
        sc_.render.film_transparent = True
        sc_.render.image_settings.file_format = "PNG"
        sc_.render.image_settings.color_mode = "RGBA"
        sc_.render.image_settings.compression = 60
        sc_.view_settings.view_transform = "Standard"
        sc_.view_settings.look = "None"
        sc_.world = bpy.data.worlds.new("w")
        cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
        sc_.collection.objects.link(cam)
        sc_.camera = cam
        sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
        sc_.collection.objects.link(sun)
        root = bpy.data.objects.new("root", None)
        sc_.collection.objects.link(root)
        return sc_, cam, sun, root

    sc, cam, sun, root = setup_scene()
    pitch = np.radians(p["pitch"])
    shift = 0.5 - p["feet_frac"]
    cd = cam.data
    cd.type = "ORTHO"
    cd.ortho_scale = p["ortho_scale"]
    cd.shift_y = shift
    cd.clip_start, cd.clip_end = 1.0, 10000.0
    dist = 3000.0
    cam.location = (0.0, -dist * np.cos(pitch), dist * np.sin(pitch))
    cam.rotation_euler = (np.pi / 2 - pitch, 0.0, 0.0)
    bpy.context.view_layer.update()
    cam_inv = np.array(cam.matrix_world.inverted())

    def material(name, tex_path):
        m = bpy.data.materials.new(name)
        try:
            m.use_nodes = True
        except Exception:
            pass
        m.use_backface_culling = False
        try:
            m.surface_render_method = "DITHERED"
        except Exception:
            pass
        nt = m.node_tree
        nt.nodes.clear()
        n = nt.nodes
        out = n.new("ShaderNodeOutputMaterial")
        tex = n.new("ShaderNodeTexImage")
        if tex_path:
            img = bpy.data.images.load(str(tex_path), check_existing=True)
            img.alpha_mode = "STRAIGHT"
            tex.image = img
        emit = n.new("ShaderNodeEmission")
        emit.name = "emit"
        diff = n.new("ShaderNodeBsdfDiffuse")
        diff.name = "diff"
        add = n.new("ShaderNodeAddShader")
        transp = n.new("ShaderNodeBsdfTransparent")
        mix = n.new("ShaderNodeMixShader")
        clip = n.new("ShaderNodeMath")
        clip.operation = "GREATER_THAN"
        clip.inputs[1].default_value = p["alpha_clip"]
        l = nt.links
        if p.get("flip_v"):
            uv = n.new("ShaderNodeTexCoord")
            mp = n.new("ShaderNodeMapping")
            mp.inputs["Location"].default_value = (0.0, 1.0, 0.0)
            mp.inputs["Scale"].default_value = (1.0, -1.0, 1.0)
            l.new(uv.outputs["UV"], mp.inputs["Vector"])
            l.new(mp.outputs["Vector"], tex.inputs["Vector"])
        l.new(tex.outputs["Color"], emit.inputs["Color"])
        l.new(tex.outputs["Color"], diff.inputs["Color"])
        l.new(emit.outputs[0], add.inputs[0])
        l.new(diff.outputs[0], add.inputs[1])
        if p["alpha_clip"] > 0 and tex_path:
            l.new(tex.outputs["Alpha"], clip.inputs[0])
            l.new(clip.outputs[0], mix.inputs[0])
        else:
            mix.inputs[0].default_value = 1.0
        l.new(transp.outputs[0], mix.inputs[1])
        l.new(add.outputs[0], mix.inputs[2])
        l.new(mix.outputs[0], out.inputs["Surface"])
        return m

    def tex_for(src, obj_name):
        sub = {k.lower(): v for k, v in src["submesh_textures"].items()}
        t = sub.get(obj_name.lower()) or src["texture"]
        if not t and src["textures"]:
            toks = [w for w in re.split(r"[^a-z0-9]+", obj_name.lower()) if len(w) > 2 and w not in ("mat", "tft18")]
            t = next((x for x in src["textures"] if any(w in Path(x).stem.lower() for w in toks)), None)
            t = t or next((x for x in src["textures"] if x.lower().endswith("tx_cm.png")), src["textures"][0])
        return assets / t if t else None

    def clear_unit():
        for o in list(root.children):
            bpy.data.objects.remove(o, do_unlink=True)
        for coll in (bpy.data.meshes, bpy.data.materials, bpy.data.images):
            for d in list(coll):
                if d.users == 0:
                    coll.remove(d)

    def set_light(lp):
        sun.data.energy = lp.get("sun", 0.0)
        sun.data.color = lp.get("sun_color", (1, 1, 1))
        sun.rotation_euler = (np.radians(90 - lp.get("sun_el", 55)), 0.0, np.radians(lp.get("sun_az", -40)))
        sc.world.color = [c * lp.get("world", 0.0) for c in lp.get("world_color", (1, 1, 1))]
        for m in bpy.data.materials:
            if m.node_tree and "emit" in m.node_tree.nodes:
                m.node_tree.nodes["emit"].inputs["Strength"].default_value = lp.get("emit", 1.0)

    def project(objs):
        """(top y px, bottom y px, left x px, right x px) of the shown vertices; y from the top."""
        pts = []
        for o in objs:
            me = o.data
            co = np.empty(len(me.vertices) * 3, dtype=np.float32)
            me.vertices.foreach_get("co", co)
            co = np.c_[co.reshape(-1, 3), np.ones(len(me.vertices))]
            pts.append(co @ (cam_inv @ np.array(o.matrix_world)).T)
        v = np.concatenate(pts)
        x = (v[:, 0] / p["ortho_scale"] + 0.5) * res
        y = (0.5 - (v[:, 1] / p["ortho_scale"] - shift)) * res
        return float(y.min()), float(y.max()), float(x.min()), float(x.max())

    def alpha_bbox(path):
        img = bpy.data.images.load(str(path))
        px = np.empty(res * res * 4, dtype=np.float32)
        img.pixels.foreach_get(px)
        bpy.data.images.remove(img)
        a = px.reshape(res, res, 4)[::-1, :, 3] > 0.1  # Blender rows are bottom-up
        ys, xs = np.nonzero(a)
        if not len(ys):
            return None
        return [int(xs.min()), int(ys.min()), int(xs.max()) + 1, int(ys.max()) + 1]

    ph = params_hash(p)
    out_root = Path(a.out)
    total_imgs, total_t = 0, 0.0
    for u in units:
        udir = out_root / u["api"]
        meta_p = udir / "meta.json"
        vars_ = variants(u, p)
        names = [file_name(v, y, s, ln) for v, _, _ in vars_ for y in p["yaws"] for s in p["sizes"] for ln in p["lights"]]
        if not a.force and meta_p.exists():
            old = json.loads(meta_p.read_text())
            same_vars = old.get("variants") == {v: {"source": u["sources"][si]["folder"], "meshes": m} for v, si, m in vars_}
            if old.get("params_hash") == ph and same_vars and all((udir / n).exists() for n in names):
                print(f"{u['api']}: done already ({len(names)} renders)", flush=True)
                continue
        if not a.no_game_check:
            wait_for_no_game()
        t0 = time.time()
        clear_unit()
        udir.mkdir(parents=True, exist_ok=True)
        objs_by_src = []
        for si, src in enumerate(u["sources"]):
            got = {}
            for o in src["objs"]:
                before = set(bpy.data.objects)
                bpy.ops.wm.obj_import(filepath=str(assets / o["path"]), forward_axis="NEGATIVE_Z", up_axis="Y")
                new = [x for x in bpy.data.objects if x not in before and x.type == "MESH"]
                tex = tex_for(src, o["name"])
                mat = material(f"{si}:{o['name']}", tex)
                for ob in new:
                    ob.data.materials.clear()
                    ob.data.materials.append(mat)
                    ob.parent = root
                    ob.hide_render = True
                    s = src.get("scale", 1.0) or 1.0
                    ob.scale = (s, s, s)
                got[o["name"]] = new
            objs_by_src.append(got)
        renders = {}
        n_img, t_render = 0, 0.0
        for var, si, shown in vars_:
            for ob_list in objs_by_src[si].values():
                for ob in ob_list:
                    ob.hide_render = True
            vis = [ob for nm in shown for ob in objs_by_src[si][nm]]
            for ob in vis:
                ob.hide_render = False
            for ln in p["lights"]:
                set_light(p["light_presets"][ln])
                for size in p["sizes"]:
                    for yaw in p["yaws"]:
                        root.rotation_euler = (0.0, 0.0, np.radians(YAW0 + yaw))
                        root.scale = (size, size, size)
                        bpy.context.view_layer.update()
                        fn = file_name(var, yaw, size, ln)
                        sc.render.filepath = str(udir / fn)
                        t1 = time.time()
                        bpy.ops.render.render(write_still=True)
                        t_render += time.time() - t1
                        n_img += 1
                        top, bottom, left, right = project(vis)
                        bb = alpha_bbox(udir / fn)
                        renders[fn] = {"variant": var, "yaw": yaw, "size": size, "light": ln,
                                       "mesh_top_px": round(top, 1), "mesh_bottom_px": round(bottom, 1),
                                       "mesh_left_px": round(left, 1), "mesh_right_px": round(right, 1), "alpha_bbox": bb,
                                       "clipped": bool(top < 0 or left < 0 or right > res or bottom > res)}
        dt = time.time() - t0
        total_imgs += n_img
        total_t += t_render
        feet = [res / 2, res * (1 - p["feet_frac"])]
        meta = {"api": u["api"], "name": u["name"], "kind": u["kind"], "params": p, "params_hash": ph, "res": res,
                "feet_px": feet, "px_per_unit": res / p["ortho_scale"],
                "skin_scale": [s.get("scale", 1.0) for s in u["sources"]],
                "variants": {v: {"source": u["sources"][si]["folder"], "meshes": m} for v, si, m in vars_},
                "seconds": round(dt, 2), "seconds_per_render": round(t_render / max(n_img, 1), 3), "renders": renders}
        tmp = meta_p.with_suffix(".tmp")
        tmp.write_text(json.dumps(meta, indent=1))
        tmp.replace(meta_p)
        print(f"{u['api']}: {len(vars_)} variants, {n_img} renders in {dt:.1f}s ({t_render / max(n_img, 1):.3f}s/render)", flush=True)
    if total_imgs:
        print(f"total {total_imgs} renders, {total_t / total_imgs:.3f}s/render", flush=True)


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else sys.argv[1:]
    ap = argparse.ArgumentParser()
    ap.add_argument("--manifest", default=str(DEV / "assets/manifest.json"))
    ap.add_argument("--units", default="all", help="comma-separated api names, or all")
    ap.add_argument("--out", default=str(DEV / "renders"))
    ap.add_argument("--params", default=str(HERE / "params.json"))
    ap.add_argument("--set", action="append", metavar="KEY=JSON", help="override a param, e.g. --set pitch=45 --set yaws=[0,90]")
    ap.add_argument("--force", action="store_true")
    ap.add_argument("--list", action="store_true", help="print variants and exit (no Blender needed)")
    ap.add_argument("--no-game-check", action="store_true")
    a = ap.parse_args(argv)
    p = load_params(a.params, a.set)
    manifest = json.loads(Path(a.manifest).read_text())
    units = pick_units(manifest, a.units)
    if a.list:
        n = 0
        for u in units:
            vs = variants(u, p)
            n += len(vs)
            print(f"{u['api']:24} {', '.join(v for v, _, _ in vs)}")
        per = len(p["yaws"]) * len(p["sizes"]) * len(p["lights"])
        print(f"{len(units)} units, {n} variants, {n * per} renders at {per} per variant")
        return
    blender_main(a, p, manifest, units)


if __name__ == "__main__":
    main()
