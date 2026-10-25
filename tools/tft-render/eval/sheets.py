"""Contact sheets for labelling the eval crops (written by `eval_crops`): per crop of a group, the
capture around its bar at 3x, with its index (order in crops.jsonl within the group), ★, stage and
frame. Ten per sheet.

labels.jsonl (beside this file): one line per combat crop, keyed by game + frame + bar x/y:
`api` is the unit's apiName, "?" when unsure (excluded from scoring) or "-" when no unit shows
under the bar (spawning in, behind the "Combat" banner or a name plate).

py -3.12 -I tools/tft-render/eval/sheets.py <eval dir>/<game> <captures dir>/<game> <group> <out dir> [i,j,...]
"""
import json, os, sys
from PIL import Image, ImageDraw

Z, CTX, COLS, PER = 3, (-30, 5, 95, 165), 5, 10  # window around the bar's left end (2560 px)


def main():
    game, caps, group, out = sys.argv[1:5]
    recs = [json.loads(l) for l in open(os.path.join(game, 'crops.jsonl'), encoding='utf-8')]
    recs = [r for r in recs if r['group'] == group]
    idx = [int(i) for i in sys.argv[5].split(',')] if len(sys.argv) > 5 else list(range(len(recs)))
    os.makedirs(out, exist_ok=True)
    tw, th = (CTX[2] - CTX[0]) * Z, (CTX[3] - CTX[1]) * Z
    for s in range(0, len(idx), PER):
        chunk = idx[s:s + PER]
        sheet = Image.new('RGB', (COLS * tw, (len(chunk) + COLS - 1) // COLS * (th + 16)), (20, 20, 20))
        d = ImageDraw.Draw(sheet)
        for k, i in enumerate(chunk):
            r = recs[i]
            im = Image.open(os.path.join(caps, r['frame'] + '.png')).convert('RGB')
            x, y = r['x'], r['y']
            t = im.crop((int(x + CTX[0]), int(y + CTX[1]), int(x + CTX[2]), int(y + CTX[3]))).resize((tw, th), Image.LANCZOS)
            X, Y = (k % COLS) * tw, (k // COLS) * (th + 16)
            sheet.paste(t, (X, Y))
            d.text((X + 2, Y + th + 2), f"{i} *{r['star']} {r.get('stage')} {r.get('who') or ''} {r['frame'][-5:]}"[:60], fill=(255, 255, 0))
        p = os.path.join(out, f'{group}_{chunk[0]:03d}.png')
        sheet.save(p)
        print(p)


if __name__ == '__main__':
    main()
