"""Fits the win-probability table from features.py's rows and writes it only when the gates pass.

py -3.12 -I tools/winprob/fit.py [--rows rows.npz] [--out crates/coach/winprob.toml] [--cases fixtures/coach/winprob_cases.json] [--l2 1.0]

Logistic regression on a hat-function basis (feature × knot, the coefficient runs straight
between knots as `winprob::beta` reads it), L2 penalty, Newton steps, every game weighted the
same. Held out: the 20 % of games features.py marked by match id. Each feature's sign is
checked on five disjoint fifths of the training games; a feature whose sign is not the same in
all five is dropped and the rest refitted. Gates (spec "Testing"): ECE at most 2 points overall
and 3 in every 5-minute bucket from 5:00 that has enough held-out games, log loss below the
gold-only model. Exit status 1 and nothing written when a gate fails.
"""
import argparse, json, math, os, sys, tomllib
from pathlib import Path

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import features as F  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
OUT = REPO / "crates" / "coach" / "winprob.toml"
CASES = REPO / "fixtures" / "coach" / "winprob_cases.json"
ECE_MAX = 2.0
ECE_BUCKET_MAX = 3.0
BUCKET_W = 5
GATE_FROM = 5
BUCKET_LAST = 40
BUCKET_GATE_GAMES = 1500
ACCURACY_AT = (10, 15, 20, 25)
BINS = 10
N_CASES = 200
L2 = 1.0
CHUNK = 65536
NEWTON_ITERS = 60
NEWTON_TOL = 1e-9


def hat(minutes, knots=F.KNOTS):
    eye = np.eye(len(knots))
    return np.stack([np.interp(minutes, knots, eye[j]) for j in range(len(knots))], axis=1)


def beta(knots, values, minute):
    """`winprob::beta`."""
    n = min(len(knots), len(values))
    at = [i for i in range(n) if knots[i] <= minute]
    if not at:
        return values[0] if values else 0.0
    i = at[-1]
    return values[i] + (values[i + 1] - values[i]) * (minute - knots[i]) / (knots[i + 1] - knots[i]) if i + 1 < n else values[i]


def sigmoid(z):
    return 1.0 / (1.0 + np.exp(-z))


def _design(X, H):
    return (X[:, :, None] * H[:, None, :]).reshape(len(X), -1)


def newton(X, H, y, w, lam):
    """Coefficients (features × knots) minimising Σ w·logloss + lam/2·|v·rms(x)|²: the penalty is on
    each feature's root-mean-square scale, so it does not depend on the feature's units."""
    rms = np.sqrt(np.mean(X ** 2, axis=0))
    rms = np.where(rms > 0, rms, 1.0)
    X = X / rms
    d = X.shape[1] * H.shape[1]
    v = np.zeros(d)

    def loss_grad_hess(v, need_hess=True):
        loss, g, A = 0.5 * lam * v @ v, lam * v, lam * np.eye(d) if need_hess else None
        for s in range(0, len(y), CHUNK):
            Z = _design(X[s:s + CHUNK], H[s:s + CHUNK])
            z = Z @ v
            p, ws, ys = sigmoid(z), w[s:s + CHUNK], y[s:s + CHUNK]
            loss += np.sum(ws * (np.logaddexp(0.0, z) - ys * z))
            if need_hess:
                g += Z.T @ (ws * (p - ys))
                A += (Z * (ws * p * (1.0 - p))[:, None]).T @ Z
        return loss, g, A

    loss, g, A = loss_grad_hess(v)
    for _ in range(NEWTON_ITERS):
        step = np.linalg.solve(A, g)
        t = 1.0
        while True:
            nv = v - t * step
            nloss = loss_grad_hess(nv, need_hess=False)[0]
            if nloss <= loss + 1e-12 or t < 1e-6:
                break
            t *= 0.5
        v = nv
        if np.max(np.abs(t * step)) < NEWTON_TOL:
            break
        loss, g, A = loss_grad_hess(v)
    return v.reshape(X.shape[1], H.shape[1]) / rms[:, None]


def predict(X, H, V):
    return sigmoid(((X @ V) * H).sum(axis=1))


def mean_beta(X, H, V):
    """Each feature's coefficient averaged over the rows, weighted by |x|; 0 without support."""
    a = np.abs(X)
    num = (a * (H @ V.T)).sum(axis=0)
    den = a.sum(axis=0)
    return np.where(den > 0, num / np.where(den > 0, den, 1.0), 0.0)


def ece(y, p, bins=BINS):
    b = np.minimum((p * bins).astype(int), bins - 1)
    total = 0.0
    for i in range(bins):
        m = b == i
        if m.any():
            total += m.sum() / len(p) * abs(p[m].mean() - y[m].mean())
    return 100.0 * total


def scores(y, p):
    q = np.clip(p, 1e-12, 1 - 1e-12)
    return dict(rows=len(y), log_loss=float(-np.mean(y * np.log(q) + (1 - y) * np.log(1 - q))), brier=float(np.mean((p - y) ** 2)),
                ece=float(ece(y, p)), accuracy=float(np.mean((p > 0.5) == (y > 0.5))))


def buckets():
    edges = list(range(0, BUCKET_LAST + 1, BUCKET_W))
    return [(lo, hi if hi != BUCKET_LAST + BUCKET_W else math.inf) for lo, hi in zip(edges, edges[1:] + [BUCKET_LAST + BUCKET_W])]


def evaluate(minute, y, p, base, game):
    """Overall and per-bucket scores, the reliability table, accuracy at ACCURACY_AT and the gate result."""
    out = dict(overall=scores(y, p), baseline=scores(y, base), buckets=[], fails=[], ungated=[])
    for lo, hi in buckets():
        m = (minute >= lo) & (minute < hi)
        if not m.any():
            continue
        games = len(np.unique(game[m]))
        row = dict(lo=lo, hi=hi, games=games, model=scores(y[m], p[m]), baseline=scores(y[m], base[m]))
        out["buckets"].append(row)
        if lo >= GATE_FROM:
            if games < BUCKET_GATE_GAMES:
                out["ungated"].append(row)
            elif row["model"]["ece"] > ECE_BUCKET_MAX:
                out["fails"].append(f"ECE {row['model']['ece']:.2f} > {ECE_BUCKET_MAX} at {lo}-{hi} min")
    b = np.minimum((p * BINS).astype(int), BINS - 1)
    out["reliability"] = [(i, int((b == i).sum()), float(p[b == i].mean()), float(y[b == i].mean())) for i in range(BINS) if (b == i).any()]
    acc = []
    for t in ACCURACY_AT:
        m = np.abs(minute - t) < 0.5
        acc.append(float(np.mean((p[m] > 0.5) == (y[m] > 0.5))) if m.any() else float("nan"))
    out["accuracy_at"] = acc
    if out["overall"]["ece"] > ECE_MAX:
        out["fails"].append(f"overall ECE {out['overall']['ece']:.2f} > {ECE_MAX}")
    if out["overall"]["log_loss"] >= out["baseline"]["log_loss"]:
        out["fails"].append(f"log loss {out['overall']['log_loss']:.4f} not below gold-only {out['baseline']['log_loss']:.4f}")
    return out


def print_report(ev, keys, V, dropped, fold_signs):
    print("\ncoefficients (per knot " + ", ".join(str(k) for k in F.KNOTS) + " min):")
    for k, row in zip(keys, V):
        print(f"  {k:<8} " + " ".join(f"{x:8.4f}" for x in row))
    print("fold signs (|x|-weighted mean coefficient on five disjoint fifths):")
    for k, s in fold_signs.items():
        print(f"  {k:<8} " + " ".join(f"{x:+.4f}" for x in s) + ("   DROPPED" if k in dropped else ""))
    o, b = ev["overall"], ev["baseline"]
    print(f"\nheld out: {o['rows']} rows; model log loss {o['log_loss']:.4f} brier {o['brier']:.4f} ECE {o['ece']:.2f} acc {o['accuracy']:.3f}"
          f" | gold-only {b['log_loss']:.4f} {b['brier']:.4f} {b['ece']:.2f} {b['accuracy']:.3f}")
    print(f"{'minutes':>8} {'games':>6} {'rows':>7} | {'logloss':>7} {'brier':>6} {'ECE':>5} {'acc':>5} | gold-only {'logloss':>7} {'brier':>6} {'ECE':>5} {'acc':>5}")
    for r in ev["buckets"]:
        m, g = r["model"], r["baseline"]
        span = f"{r['lo']}-{'' if r['hi'] == math.inf else r['hi']}"
        print(f"{span:>8} {r['games']:>6} {m['rows']:>7} | {m['log_loss']:7.4f} {m['brier']:6.4f} {m['ece']:5.2f} {m['accuracy']:5.3f} |"
              f"           {g['log_loss']:7.4f} {g['brier']:6.4f} {g['ece']:5.2f} {g['accuracy']:5.3f}")
    print("reliability:\n     p      rows  mean p   won")
    for i, n, mp, wy in ev["reliability"]:
        print(f"  {i * 10:>3}-{(i + 1) * 10:<3} {n:>7} {100 * mp:6.1f}% {100 * wy:5.1f}%")
    print("accuracy at " + ", ".join(f"{t} min {a:.3f}" for t, a in zip(ACCURACY_AT, ev["accuracy_at"])))
    for r in ev["ungated"]:
        print(f"not gated: {r['lo']}-{'' if r['hi'] == math.inf else r['hi']} min has {r['games']} held-out games (< {BUCKET_GATE_GAMES}), ECE {r['model']['ece']:.2f}")


def fmt(v):
    if isinstance(v, str):
        return json.dumps(v)
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, int):
        return str(v)
    if isinstance(v, float):
        return repr(float(f"{v:.6g}"))
    return "[" + ", ".join(fmt(x) for x in v) + "]"


HEADER = """# Win probability coefficients, written by tools/winprob/fit.py (see
# docs/superpowers/specs/2026-11-11-win-probability-design.md). `games = 0`: no fit yet, and the
# coach shows no win probability.
#
# Per mode table: one array per feature key (winprob::KEYS), a value per knot (minutes); a key
# left out counts 0. `[rift.metrics]` / `[aram.metrics]`: held-out log_loss, brier, ece (points)
# and accuracy at 10 / 15 / 20 / 25 min.
"""


def toml_text(patch, games, knots, tables):
    lines = [HEADER.rstrip("\n"), f"patch = {fmt(patch)}", f"games = {games}", "knots = [" + ", ".join(str(int(k)) if float(k).is_integer() else fmt(float(k)) for k in knots) + "]"]
    for mode in ("rift", "aram"):
        t = tables.get(mode, {})
        lines += ["", f"[{mode}]"]
        lines += [f"{k} = {fmt([float(x) for x in t[k]])}" for k in F.KEYS if k in t]
        if isinstance(t.get("metrics"), dict):
            lines += ["", f"[{mode}.metrics]"] + [f"{k} = {fmt(v)}" for k, v in t["metrics"].items()]
    return "\n".join(lines) + "\n"


def check_schema(model):
    """What `winprob::Model` needs from the file, checked in Python."""
    assert isinstance(model["patch"], str) and isinstance(model["games"], int) and model["games"] >= 0
    knots = model["knots"]
    assert all(isinstance(k, (int, float)) for k in knots) and list(knots) == sorted(knots)
    for mode in ("rift", "aram"):
        for k, v in model.get(mode, {}).items():
            if k == "metrics":
                assert all(isinstance(v[f], (int, float)) for f in ("log_loss", "brier", "ece")), v
                assert all(isinstance(x, (int, float)) for x in v.get("accuracy", []))
            else:
                assert k in F.KEYS, k
                assert len(v) == len(knots) and all(isinstance(x, (int, float)) for x in v), (k, v)


def case_p(model, case):
    """The parity test's probability for one case, from the parsed TOML."""
    table = model.get(case["mode"], {})
    z = sum(beta(model["knots"], table[k], case["minute"]) * case["x"][k] for k in F.KEYS if k in table and k in case["x"])
    return 1.0 / (1.0 + math.exp(-z))


def inside_repo(p):
    try:
        Path(p).resolve().relative_to(REPO)
        return True
    except ValueError:
        return False


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--rows", default=str(F.ROWS))
    ap.add_argument("--out", default=str(OUT))
    ap.add_argument("--cases", default=str(CASES))
    ap.add_argument("--l2", type=float, default=L2)
    a = ap.parse_args(argv)
    with np.load(a.rows, allow_pickle=False) as z:
        d = {k: z[k] for k in z.files}
    mode, patch, source = str(d["mode"]), str(d["patch"]), str(d["source"])
    if source != "riot" and (inside_repo(a.out) or inside_repo(a.cases)):
        sys.exit(f"rows are {source!r}: they must not be written into the repo ({a.out}, {a.cases})")
    if list(d["keys"]) != list(F.KEYS):
        sys.exit(f"rows have keys {list(d['keys'])}, expected {list(F.KEYS)}")
    game, minute, y, X, test, folds = d["game"], d["minute"], d["y"], d["X"], d["test"], d["fold"]
    w = 1.0 / np.bincount(game)[game]
    H = hat(minute)
    keys = list(F.MODE_KEYS[mode])
    col = [F.KEYS.index(k) for k in keys]
    tr = ~test
    train_games = len(np.unique(game[tr]))
    print(f"{mode} {patch}: {train_games} training games ({tr.sum()} rows), {len(np.unique(game[test]))} held out ({test.sum()} rows)")

    fold_signs = {k: [] for k in keys}
    for f in range(F.FOLDS):
        m = tr & (folds == f)
        if not m.any():
            sys.exit(f"fold {f} has no training games")
        mb = mean_beta(X[m][:, col], H[m], newton(X[m][:, col], H[m], y[m], w[m], a.l2))
        for k, b in zip(keys, mb):
            fold_signs[k].append(float(b))
    dropped = [k for k, s in fold_signs.items() if not (all(x > 0 for x in s) or all(x < 0 for x in s))]
    kept = [k for k in keys if k not in dropped]
    if "gold" not in kept:
        sys.exit("gold has no stable sign: the rows are wrong, not the model")
    kc = [F.KEYS.index(k) for k in kept]
    V = newton(X[tr][:, kc], H[tr], y[tr], w[tr], a.l2)
    gc = [F.KEYS.index("gold")]
    Vg = newton(X[tr][:, gc], H[tr], y[tr], w[tr], a.l2)
    p = predict(X[test][:, kc], H[test], V)
    base = predict(X[test][:, gc], H[test], Vg)
    ev = evaluate(minute[test], y[test], p, base, game[test])
    print_report(ev, kept, V, dropped, fold_signs)
    if ev["fails"]:
        print("\nGATES FAILED, nothing written:\n  " + "\n  ".join(ev["fails"]))
        return 1

    out, cases_path = Path(a.out), Path(a.cases)
    old = tomllib.loads(out.read_text(encoding="utf-8")) if out.exists() else {}
    tables = {m: old.get(m, {}) for m in ("rift", "aram") if m != mode and old.get(m) and list(old.get("knots", [])) == list(F.KNOTS)}
    o = ev["overall"]
    metrics = dict(log_loss=o["log_loss"], brier=o["brier"], ece=o["ece"], accuracy=ev["accuracy_at"], baseline_log_loss=ev["baseline"]["log_loss"],
                   test_games=len(np.unique(game[test])), test_rows=int(test.sum()), dropped=dropped)
    tables[mode] = {k: list(V[i]) for i, k in enumerate(kept)} | {"metrics": metrics}
    top = (patch, train_games) if mode == "rift" or not any(k != "metrics" for k in old.get("rift", {})) else (old["patch"], old["games"])
    text = toml_text(top[0], top[1], F.KNOTS, tables)
    model = tomllib.loads(text)
    check_schema(model)

    rng = np.random.default_rng(0)
    ti = np.flatnonzero(test)
    pick = np.sort(rng.choice(ti, size=min(N_CASES, len(ti)), replace=False))
    cases = [dict(mode=mode, minute=float(minute[i]), x={k: float(X[i, F.KEYS.index(k)]) for k in kept}) for i in pick]
    for c in cases:
        c["p"] = case_p(model, c)
    if cases_path.exists():
        cases = [c for c in json.loads(cases_path.read_text(encoding="utf-8")) if c.get("mode") != mode] + cases
    out.parent.mkdir(parents=True, exist_ok=True)
    cases_path.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(text, encoding="utf-8")
    cases_path.write_text(json.dumps(cases, indent=1) + "\n", encoding="utf-8")
    print(f"\ngates pass; wrote {out} and {len(cases)} cases to {cases_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
