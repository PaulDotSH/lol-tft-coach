"""Side-by-side sheet: real unit crops next to renders cut the way the app cuts a crop (rough Python preview of T5).

Usage: py -3.12 -I tools/tft-render/sheet.py --out sheet.png --unit DA_18_Ahri=real1.png,real2.png --unit DA_18_Sett= ...
       [--renders DIR] [--game-ppu 0.6] [--bar-gap 35] [--bg path.png] [--pick base_000_1.00_warm.png,...]

Each render is scaled from its `px_per_unit` to `--game-ppu` (screen px per model unit at 2560 width), dropped on a
background with its feet at a fixed point, a bar is assumed `--bar-gap` px above the mesh top, and the `units::CROP` window
(-18..82, +30..150 from the bar's left end; the unit's centre is taken at +32) is cut, then everything is shown at 2x.
"""
import argparse, json, os
from pathlib import Path
from PIL import Image, ImageDraw

CROP = (-18, 30, 82, 150)
CENTRE = 32
DEV = Path(os.environ.get("LOCALAPPDATA", ".")) / "lol-coach/dev/render"
ZOOM = 2


def cut(render, meta, r, ppu, gap, bg):
    k = ppu / meta["px_per_unit"]  # the star size is already in the render; only the camera scale changes
    im = render.resize((max(1, round(render.width * k)), max(1, round(render.height * k))), Image.LANCZOS)
    fx, fy = meta["feet_px"][0] * k, meta["feet_px"][1] * k
    canvas = bg.copy() if bg else Image.new("RGBA", (600, 600), (70, 72, 150, 255))
    ox, oy = canvas.width // 2 - round(fx), int(canvas.height * 0.75) - round(fy)
    canvas.alpha_composite(im, (ox, oy))
    bar_x = ox + fx - CENTRE
    bar_y = oy + r["mesh_top_px"] * k - gap
    return canvas.crop((round(bar_x + CROP[0]), round(bar_y + CROP[1]), round(bar_x + CROP[2]), round(bar_y + CROP[3])))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--renders", default=str(DEV / "renders"))
    ap.add_argument("--unit", action="append", required=True, help="api=real1.png,real2.png (real crops may be empty)")
    ap.add_argument("--pick", default="", help="render files per unit (default: base, yaws 0/30/330, size 1, every light)")
    ap.add_argument("--game-ppu", type=float, default=0.6)
    ap.add_argument("--bar-gap", type=float, default=35)
    ap.add_argument("--bg", help="background image (a 600x600 board patch); default flat arena blue")
    a = ap.parse_args()
    bg = Image.open(a.bg).convert("RGBA") if a.bg else None
    w, h = (CROP[2] - CROP[0]) * ZOOM, (CROP[3] - CROP[1]) * ZOOM
    rows = []
    for spec in a.unit:
        api, _, reals = spec.partition("=")
        udir = Path(a.renders) / api
        meta = json.loads((udir / "meta.json").read_text())
        picks = a.pick.split(",") if a.pick else [f for f, r in meta["renders"].items()
                                                  if r["variant"] == "base" and r["yaw"] in (0, 30, 330) and r["size"] == 1.0]
        tiles = [Image.open(p).convert("RGBA") for p in reals.split(",") if p]
        tiles.append(None)  # gap between real and rendered
        tiles += [cut(Image.open(udir / f).convert("RGBA"), meta, meta["renders"][f], a.game_ppu, a.bar_gap, bg) for f in picks if f in meta["renders"]]
        rows.append((api, tiles))
    cols = max(len(t) for _, t in rows)
    sheet = Image.new("RGB", (cols * (w + 4) + 4, len(rows) * (h + 22) + 4), "white")
    d = ImageDraw.Draw(sheet)
    for ri, (api, tiles) in enumerate(rows):
        y = 4 + ri * (h + 22)
        d.text((4, y), api, fill="black")
        for ci, t in enumerate(tiles):
            if t is not None:
                sheet.paste(t.resize((w, h), Image.NEAREST).convert("RGB"), (4 + ci * (w + 4), y + 16))
    sheet.save(a.out)
    print(a.out)


if __name__ == "__main__":
    main()
