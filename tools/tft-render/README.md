# tft-render

Renders of the Set 18 unit models, used to seed the TFT looks library (plan
`docs/superpowers/plans/2026-10-25-tft-render-recognition-plan.md`, T2/T3/T5/T6).

Dev data lives outside the repo in `%LOCALAPPDATA%\lol-coach\dev\render\`: `assets\` (downloads + `manifest.json`),
`renders\` (PNG + `meta.json` per unit). Python is the system 3.12 with `requests pillow numpy`; Blender 5.2 runs headless.

**No renders while a game runs.** `render.py` checks `tasklist` for `League of Legends.exe` / `TFTClient-Win64-Shipping.exe`
before each unit and waits (polls every 3 min) while one runs; `--no-game-check` turns that off.

## 1. Assets

```
py -3.12 -I tools/tft-render/fetch.py            # all units; re-running downloads nothing new
py -3.12 -I tools/tft-render/fetch.py --refresh  # re-read Community Dragon listings and skin bins
```

- Units: Set 18 champions with traits and cost 1-5; the folder comes from the unit's icon path
  (`assets/characters/tft18_ahri/...`). Summons with their own folder (Azir soldier, Yorick ghoul, Zyra plants, Maokai
  sapling, raptor and krug minis) get `kind: summon`, api = folder. Second-form folders (`tft18_gnarbig`,
  `tft18_elisespider`, `tft18_nidaleecougar`) are extra mesh sources of the parent unit.
- Per folder: every OBJ of `skins/base/<skin>/`, every PNG of `skins/base/`, and `data/characters/<characterName>/skins/skin0.bin.json`,
  which gives the meshes hidden by default (`initialSubmeshToHide`), `skinScale`, the base texture and per-submesh textures
  (some live in LoL skin folders; saved under `<folder>\ext\`), and `.../animations/skin0.bin.json`, whose `Idle*` clips'
  submesh events hide or show meshes (Ahri's idle hides her single tail, Fiddlesticks' shows his lantern and demon arm).
- `manifest.json`: `units[]` with `api, name, cost, kind, folder, sources[{folder, objs[{name, path, verts, bbox}], textures,
  texture, submesh_textures, hide, idle_hide, idle_show, scale, texture_guess}]`; units sharing a folder (the nine Lux variants) are `alias_of` the
  first; plus `missing`, `skipped` (PvE, dummies, armory keys) and `not_in_set` (tft18 folders the set data doesn't name).

## 2. Renders

```
set BLENDER="C:\Program Files\Blender Foundation\Blender 5.2\blender.exe"
%BLENDER% -b -P tools/tft-render/render.py -- --units DA_18_Ahri,DA_18_Sett,tft18_azirsoldier
%BLENDER% -b -P tools/tft-render/render.py -- --units all                     # full run, resumable per unit
%BLENDER% -b -P tools/tft-render/render.py -- --units DA_18_Ahri --out %LOCALAPPDATA%\lol-coach\dev\render\sweep\p45 ^
    --set pitch=45 --set "yaws=[0,60]" --set "sizes=[1.0]" --set "lights=[\"flat\"]"   # calibration sweep point
py -3.12 -I tools/tft-render/render.py --list [--units ...]                   # variants and render count, no Blender
```

Options: `--params P.json` (default `params.json` here), `--set key=<json>` overrides any param, `--out DIR`
(default `renders\`), `--manifest`, `--force` (ignore finished units). A unit counts as finished when its `meta.json`
has the same params hash and all its PNGs exist.

### Params (`params.json`)

| key | meaning |
|---|---|
| `pitch` | camera elevation in degrees (0 = side view, 90 = top-down) |
| `ortho_scale` | orthographic view size in model units (OBJ units, Ahri is ~170 tall); fixed for all units so sizes stay true |
| `feet_frac` | the unit's origin (feet) is drawn at x = res/2, y = res·(1 − feet_frac) from the top |
| `res` | square image size in px |
| `samples` | EEVEE samples |
| `yaws` | model turns in degrees; 0 faces the camera |
| `sizes` | model scale factors on top of the skin's `skinScale` (★1 and the star-up size) |
| `lights`, `light_presets` | presets: `emit` (unlit share: emission = texture × emit), `sun`, `sun_color`, `sun_el`, `sun_az`, `world` (ambient), `world_color` |
| `flip_v` | flip OBJ texture V (needed: Community Dragon OBJs keep League's top-left UV origin) |
| `alpha_clip` | texture alpha below this is cut out (0 = ignore texture alpha) |
| `form_frac`, `form_height`, `form_modes` | variant rules, below |

### Variants

Per mesh source: `base` = the meshes shown by default (not in `initialSubmeshToHide` or hidden by an Idle clip, unless an
Idle clip shows them; never `VFX`/`Recall`/`Return`/`Smear`).
Extra form folders are their own base (`gnarbig`, `elisespider`, `nidaleecougar`). A hidden mesh with ≥ `form_frac` of the
largest shown mesh's vertices gives `with-<mesh>` (base + it) and, when it is ≥ `form_height` of the base's height,
`only-<mesh>` (alone; Ahri's fox and mid form). T6 drops variants that never win a real crop.

### Output

`renders\<api>\<variant>_<yaw>_<size>_<light>.png` (RGBA, transparent background) and `meta.json`:
`feet_px [x, y]`, `px_per_unit` (res / ortho_scale, before `size`/`skin_scale`), `skin_scale`, `variants`, timing, and
per render `variant, yaw, size, light, mesh_top_px, mesh_bottom_px, mesh_left_px, mesh_right_px` (projected vertices of
the shown meshes, y from the top), `alpha_bbox [x0, y0, x1, y1]` and `clipped` (mesh leaves the frame).

## 3. Preview sheet

```
py -3.12 -I tools/tft-render/sheet.py --out sheet.png --game-ppu 0.5 --bar-gap 35 ^
    --unit DA_18_Sett=real1.png,real2.png --unit DA_18_Ahri= [--pick base_000_1.00_flat.png,...]
```

Real crops (from `TFT_CROPS` / the eval set) beside renders scaled to `--game-ppu` and cut with `units::CROP` below a bar
assumed `--bar-gap` px above the mesh top. A rough Python stand-in for T5's compositing, for eyeballing only.

## Known limits

- **No per-unit game scale in the data.** Model units are not screen-consistent across units: at one scale Sett matches his
  real crops at about 0.5 px/unit while Ahri needs about 0.9 (Gnar, a small yordle in game, is as tall as Ahri in model
  units). TFT's character records (base scale, health bar height) are not on Community Dragon for `DA_*` units, so scale
  and bar offset need fitting per unit from real crops, or renders normalised by height.

- Bind pose only (Community Dragon has no `.skl`/`.anm` for these): arms out, weapons where the rig puts them (Lillia's
  staff floats beside her), Ahri's nine tails fan out sideways.
- Materials are approximated by their colour texture only. Shader effects are lost: Ahri's tails are teal in the texture
  but pink in game; Sett's spirit badgers show in the bind pose but hardly in game.
- Yaw 0 faces the camera for most units; a few models face the other way (Azir soldier).
- Lux's nine trait variants share one mesh and texture; only `DA_Lux18_Base` is rendered.
- Zyra has no texture in her skin bin; her mesh is LoL Zyra's (same vertices and UVs), so LoL Zyra's texture is used. The
  Maokai sapling uses Maokai's texture as a guess (`texture_guess: true`).
- Whether each summon shows an HP bar in game is not checked.
