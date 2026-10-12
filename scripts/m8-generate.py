#!/usr/bin/env python3
"""Milestone 8 case generator (benchmarks/evidence-outcomes/protocol.json).

  python scripts/m8-generate.py --cache DIR --output NEW_DIR [--audit-only]

Downloads (or reuses) the five public archives, verifies their SHA-256, builds the
eligible prediction units of each dataset, and then, unless --audit-only:
fits the frozen models, computes material errors, generates every case of the
four claim families, and writes

  NEW_DIR/cases.json   claims and evidence only (no truth), for every classifier
  NEW_DIR/truth.json   sealed truth and case metadata, for the scorer only
  NEW_DIR/units.json   per-dataset, per-period and per-stratum counts and rates

--audit-only writes NEW_DIR/data-audit.json with eligibility and cluster counts
and fits no model. Deterministic given the archives (single-threaded BLAS).
"""
import os

for _v in ("OMP_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS"):
    os.environ.setdefault(_v, "1")

import argparse
import csv
import datetime as dt
import gzip
import hashlib
import io
import json
import math
import sys
import urllib.request
import zipfile
from pathlib import Path

import numpy as np
from scipy import stats
from sklearn.linear_model import LogisticRegression

ROOT = Path(__file__).resolve().parents[1]
PROTOCOL = json.loads((ROOT / "benchmarks/evidence-outcomes/protocol.json").read_text(encoding="utf-8"))
SEED = 80808
BOOT = 2000
GAMMA = 0.95
COVERAGE_LEVEL = 0.90
COVERAGE_TOLERANCE = 0.02
MONTHS = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"]


# ---------------------------------------------------------------------------
# Archives


def archive(cache, key):
    meta = PROTOCOL["datasets"][key]
    path = Path(cache) / f"{key}.zip"
    if not path.exists():
        path.parent.mkdir(parents=True, exist_ok=True)
        with urllib.request.urlopen(meta["archive"], timeout=120) as r:
            data = r.read()
        path.write_bytes(data)
    data = path.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    if digest != meta["archive_sha256"]:
        raise SystemExit(f"{key}: archive SHA-256 {digest} does not match the protocol")
    return zipfile.ZipFile(io.BytesIO(data))


def rng_for(*parts):
    h = hashlib.sha256("|".join(map(str, (SEED,) + parts)).encode()).digest()
    return np.random.default_rng(int.from_bytes(h[:8], "little"))


def period_of(day, periods):
    for name, (a, b) in periods.items():
        if a <= day <= b:
            return name
    return None


def iso_periods(key):
    return {k: (dt.date.fromisoformat(v[0]), dt.date.fromisoformat(v[1]))
            for k, v in PROTOCOL["datasets"][key]["periods"].items()}


# ---------------------------------------------------------------------------
# Loaders: each returns a dict of aligned arrays, sorted in time (archive) order.
#   time: sortable int; period; stratum; cluster; y; X (features); kind
#   regression extras: date (datetime.date), hour, series (site or single)


def hourly_features(hours, months, extra):
    h = np.zeros((len(hours), 24))
    h[np.arange(len(hours)), hours] = 1.0
    ang = 2 * np.pi * (np.asarray(months) - 1) / 12
    return np.column_stack([h, np.sin(ang), np.cos(ang)] + extra)


def load_aq(cache):
    outer = archive(cache, "aq")
    z = zipfile.ZipFile(io.BytesIO(outer.read("PRSA2017_Data_20130301-20170228.zip")))
    names = sorted(n for n in z.namelist() if n.endswith(".csv"))
    start = dt.datetime(2013, 3, 1)
    periods = iso_periods("aq")
    cols = {k: [] for k in ("time", "site", "y", "lags", "hour", "month", "date")}
    for s, name in enumerate(names):
        rows = list(csv.DictReader(io.TextIOWrapper(z.open(name), encoding="utf-8")))
        n = len(rows)
        v = np.full(n, np.nan)
        stamps = []
        for i, r in enumerate(rows):
            t = dt.datetime(int(r["year"]), int(r["month"]), int(r["day"]), int(r["hour"]))
            if int((t - start).total_seconds() // 3600) != i:
                raise SystemExit(f"aq {name}: row {i} is not on the hourly grid")
            stamps.append(t)
            if r["PM2.5"] != "NA":
                v[i] = float(r["PM2.5"])
        for i in range(24, n):
            lags = [v[i - 1], v[i - 2], v[i - 3], v[i - 24]]
            if np.isnan(v[i]) or any(np.isnan(lags)):
                continue
            t = stamps[i]
            cols["time"].append(i * 16 + s)
            cols["site"].append(s)
            cols["y"].append(v[i])
            cols["lags"].append(lags)
            cols["hour"].append(t.hour)
            cols["month"].append(t.month)
            cols["date"].append(t.date())
    order = np.argsort(cols["time"], kind="stable")
    site = np.asarray(cols["site"])[order]
    hour = np.asarray(cols["hour"])[order]
    month = np.asarray(cols["month"])[order]
    dates = [cols["date"][i] for i in order]
    lags = np.log1p(np.asarray(cols["lags"])[order])
    onehot = np.zeros((len(site), 12))
    onehot[np.arange(len(site)), site] = 1.0
    X = hourly_features(hour, month, [lags, onehot])
    return {
        "kind": "regression", "time": np.asarray(cols["time"])[order], "y": np.asarray(cols["y"])[order],
        "X": X, "hour": hour, "date": dates, "series": site, "n_series": 12,
        "period": [period_of(d, periods) for d in dates],
        "stratum": [d.strftime("%Y-%m") for d in dates],
        "cluster": [d.isoformat() for d in dates],
        "population": "aq ridge PM2.5 material-error", "unit": "site-hour",
    }


def hourly_series(key, stamps_values, extra_cols):
    """stamps_values: dict naive hour datetime -> value. extra_cols(t) -> list."""
    periods = iso_periods(key)
    keys = sorted(stamps_values)
    t0 = keys[0]
    idx = {int((t - t0).total_seconds() // 3600): stamps_values[t] for t in keys}
    lag_offsets = [1, 2, 24, 168]
    rows = []
    for k in sorted(idx):
        t = t0 + dt.timedelta(hours=k)
        lags = [idx.get(k - o) for o in lag_offsets]
        if any(x is None for x in lags):
            continue
        rows.append((k, t, idx[k], lags))
    hour = np.array([t.hour for _, t, _, _ in rows])
    month = np.array([t.month for _, t, _, _ in rows])
    dow = np.zeros((len(rows), 7))
    dow[np.arange(len(rows)), [t.weekday() for _, t, _, _ in rows]] = 1.0
    lags = np.log1p(np.array([l for *_, l in rows], dtype=float))
    extra = np.array([extra_cols(t) for _, t, _, _ in rows], dtype=float).reshape(len(rows), -1)
    X = hourly_features(hour, month, [lags, dow, extra])
    dates = [t.date() for _, t, _, _ in rows]
    return {
        "kind": "regression", "time": np.array([k for k, *_ in rows]), "y": np.array([v for _, _, v, _ in rows], dtype=float),
        "X": X, "hour": hour, "date": dates, "series": np.zeros(len(rows), dtype=int), "n_series": 1,
        "period": [period_of(d, periods) for d in dates],
        "stratum": [d.strftime("%Y-%m") for d in dates],
        "cluster": [d.isoformat() for d in dates],
    }


def load_tr(cache):
    z = archive(cache, "tr")
    raw = gzip.decompress(z.read("Metro_Interstate_Traffic_Volume.csv.gz")).decode("utf-8")
    first = {}
    holidays = set()
    for r in csv.DictReader(io.StringIO(raw)):
        t = dt.datetime.fromisoformat(r["date_time"])
        if r["holiday"] not in ("None", ""):
            holidays.add(t.date())
        first.setdefault(t, float(r["traffic_volume"]))
    u = hourly_series("tr", first, lambda t: [1.0 if t.date() in holidays else 0.0])
    u.update(population="tr ridge traffic-volume material-error", unit="hour")
    return u


def load_pw(cache):
    z = archive(cache, "pw")
    sums, counts = {}, {}
    with io.TextIOWrapper(z.open("household_power_consumption.txt"), encoding="utf-8") as f:
        f.readline()
        for line in f:
            p = line.split(";", 3)
            if p[2] == "?":
                continue
            d, m, y = p[0].split("/")
            t = dt.datetime(int(y), int(m), int(d), int(p[1][:2]))
            sums[t] = sums.get(t, 0.0) + float(p[2])
            counts[t] = counts.get(t, 0) + 1
    hourly = {t: sums[t] / counts[t] for t in sums if counts[t] >= 45}
    u = hourly_series("pw", hourly, lambda t: [])
    u.update(population="pw ridge household-power material-error", unit="hour")
    return u


def load_gs(cache):
    z = archive(cache, "gs")
    plan = {"train": (1, 2), "development": (3, 3), "evaluation": (4, 6), "future": (7, 10)}
    X, y, period, stratum, cluster, time = [], [], [], [], [], []
    for b in range(1, 11):
        lines = [l for l in z.read(f"Dataset/batch{b}.dat").decode("utf-8").split("\n") if l.strip()]
        for i, line in enumerate(lines):
            head, *feats = line.split()
            cls = int(head.split(";")[0])
            vec = np.zeros(128)
            for f in feats:
                k, v = f.split(":")
                vec[int(k) - 1] = float(v)
            X.append(vec)
            y.append(cls)
            period.append(next(p for p, (a, c) in plan.items() if a <= b <= c))
            stratum.append(f"batch{b}")
            cluster.append(f"batch{b}-block{i // 25:03d}")
            time.append(b * 100000 + i)
    return {"kind": "classification", "time": np.array(time), "X": np.array(X), "y": np.array(y),
            "period": period, "stratum": stratum, "cluster": cluster,
            "population": "gs logistic gas-class material-error", "unit": "measurement"}


BK_CATEGORICAL = ["job", "marital", "education", "default", "housing", "loan", "contact", "month", "day_of_week", "poutcome"]
BK_NUMERIC = ["age", "campaign", "pdays", "previous", "emp.var.rate", "cons.price.idx", "cons.conf.idx", "euribor3m", "nr.employed"]


def load_bk(cache):
    outer = archive(cache, "bk")
    z = zipfile.ZipFile(io.BytesIO(outer.read("bank-additional.zip")))
    rows = list(csv.DictReader(io.TextIOWrapper(z.open("bank-additional/bank-additional-full.csv"), encoding="utf-8"), delimiter=";"))
    run, prev, years = 0, None, []
    run_of = []
    for r in rows:
        if r["month"] != prev:
            run += 1
            prev = r["month"]
        run_of.append(run)
    if run != 26:
        raise SystemExit(f"bk: expected 26 month runs, found {run}")
    year = lambda k: 2008 if k <= 7 else (2009 if k <= 17 else 2010)
    period = ["train" if k <= 7 else "development" if k <= 10 else "evaluation" if k <= 17 else "future" for k in run_of]
    stratum = [f"{year(k)}-{MONTHS.index(r['month']) + 1:02d}" for k, r in zip(run_of, rows)]
    cluster, c, prev = [], 0, None
    for k, r in zip(run_of, rows):
        key = (k, r["day_of_week"])
        if key != prev:
            c += 1
            prev = key
        cluster.append(f"{stratum[len(cluster)]}-day{c:03d}")
    train = [r for r, p in zip(rows, period) if p == "train"]
    levels = {c: sorted({r[c] for r in train}) for c in BK_CATEGORICAL}
    num = np.array([[float(r[c]) for c in BK_NUMERIC] for r in rows])
    cat = np.array([[1.0 if r[c] == lv else 0.0 for c in BK_CATEGORICAL for lv in levels[c]] for r in rows])
    return {"kind": "binary", "time": np.arange(len(rows)), "X_num": num, "X_cat": cat,
            "y": np.array([1 if r["y"] == "yes" else 0 for r in rows]),
            "period": period, "stratum": stratum, "cluster": cluster,
            "population": "bk logistic subscription material-error", "unit": "contact"}


LOADERS = {"aq": load_aq, "tr": load_tr, "pw": load_pw, "gs": load_gs, "bk": load_bk}


# ---------------------------------------------------------------------------
# Models and material errors


def standardise(X, train):
    mu = X[train].mean(0)
    sd = X[train].std(0)
    sd[sd == 0] = 1.0
    return (X - mu) / sd


def ridge(X, y, alpha=1.0):
    A = np.column_stack([np.ones(len(X)), X])
    P = alpha * np.eye(A.shape[1])
    P[0, 0] = 0.0
    return np.linalg.solve(A.T @ A + P, A.T @ y)


def fit_predict(key, u):
    period = np.array(u["period"], dtype=object)
    train = period == "train"
    if u["kind"] == "regression":
        Z = standardise(u["X"], train)
        beta = ridge(Z[train], np.log1p(u["y"][train]))
        yhat = np.clip(np.expm1(np.column_stack([np.ones(len(Z)), Z]) @ beta), 0, None)
        floor = 0.25 * float(np.median(u["y"][train]))
        err = np.abs(yhat - u["y"]) > np.maximum(0.25 * np.abs(u["y"]), floor)
        dev = period == "development"
        F = np.column_stack([np.log1p(yhat), np.sin(2 * np.pi * u["hour"] / 24), np.cos(2 * np.pi * u["hour"] / 24)])
        em = LogisticRegression(C=1.0, max_iter=5000).fit(F[dev], err[dev])
        u.update(yhat=yhat, err=err, score=yhat, perr=em.predict_proba(F)[:, 1], tolerance_floor=floor,
                 residual=np.abs(np.log1p(u["y"]) - np.log1p(yhat)))
    elif u["kind"] == "classification":
        Z = standardise(u["X"], train)
        m = LogisticRegression(C=1.0, max_iter=5000).fit(Z[train], u["y"][train])
        yhat = m.predict(Z)
        u.update(yhat=yhat, err=yhat != u["y"], score=yhat.astype(float), categorical=True)
    else:
        Z = np.column_stack([standardise(u["X_num"], train), u["X_cat"]])
        m = LogisticRegression(C=1.0, max_iter=5000).fit(Z[train], u["y"][train])
        p = m.predict_proba(Z)[:, 1]
        u.update(yhat=p, err=(p >= 0.5).astype(int) != u["y"], score=p)
    u["err"] = np.asarray(u["err"], dtype=bool)


# ---------------------------------------------------------------------------
# Statistics


def ceil6(x):
    return min(1.0, math.ceil(x * 1e6 - 1e-9) / 1e6)


def cp_upper_one_sided(k, n, gamma=GAMMA):
    return 1.0 if k >= n else float(stats.beta.ppf(gamma, k + 1, n - k))


def cp_two_sided(k, n, level=0.95):
    a = (1 - level) / 2
    lo = 0.0 if k == 0 else float(stats.beta.ppf(a, k, n - k + 1))
    hi = 1.0 if k == n else float(stats.beta.ppf(1 - a, k + 1, n - k))
    return lo, hi


def cluster_bootstrap(errs, units, rng):
    """Percentile 95% interval of total errors / total units over resampled clusters."""
    e = np.asarray(errs, dtype=float)
    n = np.asarray(units, dtype=float)
    idx = rng.integers(0, len(e), size=(BOOT, len(e)))
    rates = e[idx].sum(1) / n[idx].sum(1)
    lo, hi = np.quantile(rates, [0.025, 0.975])
    return float(lo), float(hi)


def psi(reference, target, categorical=False):
    reference = np.asarray(reference)
    target = np.asarray(target)
    if categorical:
        cats = np.union1d(np.unique(reference), np.unique(target))
        p = np.array([(reference == c).mean() for c in cats])
        q = np.array([(target == c).mean() for c in cats])
    else:
        edges = np.unique(np.quantile(reference, np.linspace(0.1, 0.9, 9)))
        p = np.bincount(np.searchsorted(edges, reference, side="right"), minlength=len(edges) + 1) / len(reference)
        q = np.bincount(np.searchsorted(edges, target, side="right"), minlength=len(edges) + 1) / len(target)
    p = np.maximum(p, 1e-4)
    q = np.maximum(q, 1e-4)
    return float(np.sum((q - p) * np.log(q / p)))


def lag1(x):
    x = np.asarray(x, dtype=float)
    if len(x) < 3 or x[:-1].std() == 0 or x[1:].std() == 0:
        return 0.0
    return float(np.corrcoef(x[:-1], x[1:])[0, 1])


# ---------------------------------------------------------------------------
# Case construction helpers


def ordered_unique(seq):
    return list(dict.fromkeys(seq))


def clusters_of(u, idx):
    """Ordered clusters over unit indices idx -> (labels, error sums, unit counts)."""
    labels = ordered_unique(u["cluster"][i] for i in idx)
    pos = {c: j for j, c in enumerate(labels)}
    e = np.zeros(len(labels))
    n = np.zeros(len(labels))
    for i in idx:
        j = pos[u["cluster"][i]]
        e[j] += u["err"][i]
        n[j] += 1
    return labels, e, n


def metric_report(eid, u, idx, dependence, interval, source):
    k = int(u["err"][idx].sum())
    return {"kind": "metric_report", "id": eid, "metric": "material_error_rate", "population": u["population"],
            "conditions": ordered_unique(u["stratum"][i] for i in idx), "unit": u["unit"], "n": int(len(idx)),
            "events": k, "value": None, "dependence": dependence, "interval": interval, "design_effect": None,
            "source": source}


def outcome_records(eid, u, idx, dependence, population=None, design_effect=None):
    return {"kind": "outcome_records", "id": eid, "metric": "material_error_rate",
            "population": population or u["population"],
            "conditions": ordered_unique(u["stratum"][i] for i in idx), "unit": u["unit"],
            "dependence": dependence, "outcomes": [bool(u["err"][i]) for i in idx], "design_effect": design_effect}


def metric_bound(statement, u, conditions, bound, evidence):
    return {"type": "metric_bound", "id": "claim", "statement": statement, "evidence": evidence,
            "metric": "material_error_rate", "population": u["population"], "conditions": conditions,
            "comparator": "at_most", "bound": bound, "confidence": GAMMA, "claimed_value": None, "tolerance": 0.0}


def assessment(case_id, evidence, claim):
    return {"schema_version": "triad-evidence-1", "assessment_id": case_id, "evidence": evidence, "claims": [claim]}


class Sink:
    def __init__(self):
        self.cases, self.truth = [], {}

    def add(self, case_id, a, meta):
        if case_id in self.truth:
            raise SystemExit(f"duplicate case {case_id}")
        self.cases.append({"case_id": case_id, "assessment": a})
        self.truth[case_id] = meta


# ---------------------------------------------------------------------------
# Families


def family_a(key, u, sink):
    spec = PROTOCOL["families"]["A_audit_sample"]
    P = [i for i, p in enumerate(u["period"]) if p == "evaluation"]
    truth_rate = float(u["err"][P].mean())
    strata = ordered_unique(u["stratum"][i] for i in P)
    labels = ordered_unique(u["cluster"][i] for i in P)
    members = {c: [] for c in labels}
    for i in P:
        members[u["cluster"][i]].append(i)
    sizes = {"srs": [100, 400, 1600], "cluster": [4, 16, 64], "block": [4, 16, 64]}
    for design, values in sizes.items():
        for size in values:
            for rep in range(spec["replicates"]):
                rng = rng_for(key, "A", design, size, rep)
                if design == "srs":
                    idx = sorted(rng.choice(P, size=size, replace=False).tolist())
                    dependence = "independent"
                else:
                    if design == "cluster":
                        chosen = sorted(rng.choice(len(labels), size=size, replace=False).tolist())
                    else:
                        start = int(rng.integers(0, len(labels) - size + 1))
                        chosen = list(range(start, start + size))
                    idx = sorted(i for j in chosen for i in members[labels[j]])
                    dependence = "clustered" if design == "cluster" else "temporal"
                n, k = len(idx), int(u["err"][idx].sum())
                p_hat = k / n
                U = cp_upper_one_sided(k, n)
                lo, hi = cp_two_sided(k, n)
                if design == "srs":
                    aware = {"method": "clopper_pearson", "level": 0.95, "lower": lo, "upper": hi}
                else:
                    _, e, m = clusters_of(u, idx)
                    blo, bhi = cluster_bootstrap(e, m, rng_for(key, "A-boot", design, size, rep))
                    aware = {"method": "cluster_bootstrap" if design == "cluster" else "block_bootstrap",
                             "level": 0.95, "lower": blo, "upper": bhi}
                styles = {
                    "raw": outcome_records("sample", u, idx, dependence),
                    "iid": metric_report("sample", u, idx, dependence,
                                         {"method": "clopper_pearson", "level": 0.95, "lower": lo, "upper": hi},
                                         f"audit sample ({design}), i.i.d. interval"),
                    "aware": metric_report("sample", u, idx, dependence, aware,
                                           f"audit sample ({design}), design-appropriate interval"),
                }
                cluster_id = f"{key}-A-{design}-{size}-r{rep}"
                for style, ev in styles.items():
                    for m_ in (0.5, 1, 2, 4):
                        b = ceil6(p_hat + m_ * (U - p_hat))
                        cid = f"{cluster_id}-{style}-m{m_}"
                        claim = metric_bound(f"The material-error rate over the evaluation period ({strata[0]} to {strata[-1]}) is at most {b}.",
                                             u, strata, b, ["sample"])
                        sink.add(cid, assessment(cid, [ev], claim), {
                            "dataset": key, "family": "A", "cluster": cluster_id, "design": design, "size": size,
                            "rep": rep, "style": style, "template": f"m={m_}", "n": n, "events": k,
                            "truth_value": truth_rate, "threshold": b, "truth": truth_rate <= b})


def family_b(key, u, sink):
    eval_strata = ordered_unique(s for s, p in zip(u["stratum"], u["period"]) if p == "evaluation")
    all_strata = ordered_unique(s for s, p in zip(u["stratum"], u["period"]) if p in ("evaluation", "future"))
    by_stratum = {s: [] for s in all_strata}
    for i, (s, p) in enumerate(zip(u["stratum"], u["period"])):
        if p in ("evaluation", "future"):
            by_stratum[s].append(i)
    w, offsets = (1, (1, 2, 3)) if key == "gs" else (3, (1, 3, 6))
    for start in range(len(eval_strata) - w + 1):
        window = eval_strata[start:start + w]
        idx = sorted(i for s in window for i in by_stratum[s])
        n, k = len(idx), int(u["err"][idx].sum())
        p_hat = k / n
        U = cp_upper_one_sided(k, n)
        _, e, m = clusters_of(u, idx)
        lo, hi = cluster_bootstrap(e, m, rng_for(key, "B-boot", start))
        cluster_id = f"{key}-B-w{start}"
        j = all_strata.index(window[-1])
        for off in offsets:
            if j + off >= len(all_strata):
                continue
            target = all_strata[j + off]
            tidx = by_stratum[target]
            rate = float(u["err"][tidx].mean())
            ev = metric_report("window", u, idx, "temporal",
                               {"method": "block_bootstrap", "level": 0.95, "lower": lo, "upper": hi},
                               "all units of the evaluated window, moving-block bootstrap over clusters")
            shift = {"kind": "shift_diagnostic", "id": "shift", "reference": f"predictions, {window[0]} to {window[-1]}",
                     "target": f"predictions, {target}",
                     "statistic": "population stability index of predictions (10 reference-decile bins)" if not u.get("categorical")
                     else "population stability index of predicted classes",
                     "value": psi(u["score"][idx], u["score"][tidx], u.get("categorical", False)), "threshold": 0.25}
            for name, b in (("U", ceil6(U)), ("1.25p", ceil6(1.25 * p_hat)), ("1.5p", ceil6(1.5 * p_hat))):
                cid = f"{cluster_id}-t{off}-{name}"
                claim = metric_bound(f"The material-error rate in {target} is at most {b}.", u, [target], b, ["window", "shift"])
                sink.add(cid, assessment(cid, [ev, shift], claim), {
                    "dataset": key, "family": "B", "cluster": cluster_id, "design": f"window {window[0]}..{window[-1]}",
                    "target": target, "offset": off, "template": name, "n": n, "events": k,
                    "truth_value": rate, "threshold": b, "truth": rate <= b})


def conformal_q(residuals, level=COVERAGE_LEVEL):
    r = np.sort(np.asarray(residuals))
    k = math.ceil((len(r) + 1) * level)
    return math.inf if k > len(r) else float(r[k - 1])


def family_c(key, u, sink):
    eval_strata = ordered_unique(s for s, p in zip(u["stratum"], u["period"]) if p == "evaluation")
    all_strata = ordered_unique(s for s, p in zip(u["stratum"], u["period"]) if p in ("evaluation", "future"))
    by_stratum = {s: [] for s in all_strata}
    for i, (s, p) in enumerate(zip(u["stratum"], u["period"])):
        if p in ("evaluation", "future"):
            by_stratum[s].append(i)
    for M in eval_strata:
        units = by_stratum[M]
        cluster_id = f"{key}-C-{M}"
        perm = rng_for(key, "C", M).permutation(len(units))
        half = len(units) // 2
        variants = {"random_split": (sorted(units[i] for i in perm[:half]), sorted(units[i] for i in perm[half:]), M, "independent")}
        j = all_strata.index(M)
        for name, off in (("next", 1), ("plus3", 3), ("plus6", 6)):
            if j + off < len(all_strata):
                variants[name] = (units, by_stratum[all_strata[j + off]], all_strata[j + off], "temporal")
        for name, (cal, tgt, target, dependence) in variants.items():
            q = conformal_q(u["residual"][cal])
            coverage = float((u["residual"][tgt] <= q).mean())
            calib = {"kind": "calibration", "id": "calibration", "population": u["population"], "conditions": [M],
                     "condition_counts": {M: len(cal)}, "n": len(cal), "unit": u["unit"], "dependence": dependence,
                     "method": "split conformal, absolute log1p residual, level 0.90"}
            shift = {"kind": "shift_diagnostic", "id": "shift", "reference": f"calibration predictions, {M}",
                     "target": f"predictions, {target}" + (" (other random half)" if name == "random_split" else ""),
                     "statistic": "population stability index of predictions (10 reference-decile bins)",
                     "value": psi(u["score"][cal], u["score"][tgt]), "threshold": 0.25}
            cid = f"{cluster_id}-{name}"
            claim = {"type": "coverage_guarantee", "id": "claim", "evidence": ["calibration", "shift"],
                     "statement": f"Split-conformal 90% intervals calibrated on {M} cover at least 90% of outcomes in {target}"
                                  + (" (the held-out random half)." if name == "random_split" else "."),
                     "population": u["population"], "conditions": [target], "level": COVERAGE_LEVEL}
            sink.add(cid, assessment(cid, [calib, shift], claim), {
                "dataset": key, "family": "C", "cluster": cluster_id, "design": name, "target": target,
                "n": len(cal), "target_n": len(tgt), "truth_value": coverage,
                "threshold": COVERAGE_LEVEL - COVERAGE_TOLERANCE, "truth": coverage >= COVERAGE_LEVEL - COVERAGE_TOLERANCE})


def week_start(d):
    return d - dt.timedelta(days=d.weekday())


def family_d(key, u, sink):
    periods = iso_periods(key)
    a, b = periods["evaluation"]
    dev_a, dev_b = periods["development"]
    dev = [i for i, p in enumerate(u["period"]) if p == "development"]
    r_dev = float(u["err"][dev].mean())
    rho = float(np.mean([lag1(u["err"][[i for i in dev if u["series"][i] == s]]) for s in range(u["n_series"])]))
    rho = min(max(rho, 0.0), 0.9)
    deff = (1 + rho) / (1 - rho)
    counts = {}
    for i in dev:
        label = f"month-of-year {u['date'][i].month:02d}"
        counts[label] = counts.get(label, 0) + 1
    weeks = {}
    for i, d in enumerate(u["date"]):
        weeks.setdefault(week_start(d), []).append(i)
    dev_weeks = [w for w in weeks if w >= dev_a and w + dt.timedelta(days=6) <= dev_b]
    support_max = max(float(u["perr"][weeks[w]].mean()) for w in dev_weeks)
    expected = 168 * u["n_series"]
    for w in sorted(weeks):
        prev = w - dt.timedelta(days=7)
        if not (prev >= a and w + dt.timedelta(days=6) <= b) or prev not in weeks:
            continue
        coming, recent = weeks[w], weeks[prev]
        if len(coming) < 24:
            continue
        rate = float(u["err"][coming].mean())
        cluster_id = f"{key}-D-{w.isoformat()}"
        recent_pop = f"{u['population']}, previous week"
        ev = [
            {"kind": "calibration", "id": "calibration", "population": u["population"], "conditions": sorted(counts),
             "condition_counts": counts, "n": len(dev), "unit": u["unit"], "dependence": "temporal",
             "method": "development period of the frozen model"},
            {"kind": "support_check", "id": "monitor", "statistic": "mean predicted error probability, previous week",
             "value": float(u["perr"][recent].mean()), "support_max": support_max},
            {"kind": "completeness", "id": "completeness", "required": math.ceil(0.9 * expected), "available": len(recent)},
            outcome_records("recent", u, recent, "temporal", population=recent_pop, design_effect=deff),
            {"kind": "shift_diagnostic", "id": "shift", "reference": "development-period predictions",
             "target": "previous-week predictions",
             "statistic": "population stability index of predictions (10 reference-decile bins)",
             "value": psi(u["score"][dev], u["score"][recent]), "threshold": 0.25},
        ]
        for d_ in (0.02, 0.05):
            bound = round(r_dev + d_, 6)
            cid = f"{cluster_id}-d{d_}"
            claim = {"type": "operational_risk", "id": "claim",
                     "statement": f"The material-error rate over the week starting {w.isoformat()} will be at most {bound}.",
                     "evidence": [e["id"] for e in ev], "population": u["population"], "recent_population": recent_pop,
                     "condition": f"month-of-year {w.month:02d}", "bound": bound, "confidence": GAMMA,
                     "min_condition_support": 100}
            sink.add(cid, assessment(cid, ev, claim), {
                "dataset": key, "family": "D", "cluster": cluster_id, "design": f"d={d_}", "week": w.isoformat(),
                "n": len(recent), "target_n": len(coming), "truth_value": rate, "threshold": bound, "truth": rate <= bound})


# ---------------------------------------------------------------------------


def audit(key, u):
    out = {"units": len(u["period"]), "by_period": {}, "strata_by_period": {}, "clusters_by_period": {}}
    for p in ("train", "development", "evaluation", "future"):
        idx = [i for i, q in enumerate(u["period"]) if q == p]
        out["by_period"][p] = len(idx)
        out["strata_by_period"][p] = {s: sum(1 for i in idx if u["stratum"][i] == s) for s in ordered_unique(u["stratum"][i] for i in idx)}
        out["clusters_by_period"][p] = len(ordered_unique(u["cluster"][i] for i in idx))
    out["unassigned_units"] = sum(1 for q in u["period"] if q is None)
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--cache", required=True)
    ap.add_argument("--output", required=True, type=Path)
    ap.add_argument("--audit-only", action="store_true")
    ap.add_argument("--datasets", default="aq,tr,pw,gs,bk")
    args = ap.parse_args()
    args.output.mkdir(parents=False, exist_ok=False)
    keys = args.datasets.split(",")
    sink, audits, units = Sink(), {}, {}
    for key in keys:
        u = LOADERS[key](args.cache)
        for name in ("period", "stratum", "cluster"):
            u[name] = list(u[name])
        audits[key] = audit(key, u)
        print(f"{key}: {audits[key]['by_period']}", file=sys.stderr)
        if args.audit_only:
            continue
        fit_predict(key, u)
        summary = {"by_period": {}, "by_stratum": {}}
        for p in ("train", "development", "evaluation", "future"):
            idx = [i for i, q in enumerate(u["period"]) if q == p]
            summary["by_period"][p] = {"n": len(idx), "errors": int(u["err"][idx].sum())}
        for s in ordered_unique(s for s, p in zip(u["stratum"], u["period"]) if p in ("evaluation", "future")):
            idx = [i for i, q in enumerate(u["stratum"]) if q == s]
            summary["by_stratum"][s] = {"n": len(idx), "errors": int(u["err"][idx].sum())}
        if u["kind"] == "regression":
            summary["tolerance_floor"] = u["tolerance_floor"]
            weekly = {}
            for i, d in enumerate(u["date"]):
                w = week_start(d).isoformat()
                weekly.setdefault(w, [0, 0])
                weekly[w][0] += 1
                weekly[w][1] += int(u["err"][i])
            summary["by_week"] = {w: {"n": v[0], "errors": v[1]} for w, v in sorted(weekly.items())}
        units[key] = summary
        family_a(key, u, sink)
        family_b(key, u, sink)
        if u["kind"] == "regression":
            family_c(key, u, sink)
            family_d(key, u, sink)
    meta = {"protocol_version": PROTOCOL["version"], "generator_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            "archives": {k: PROTOCOL["datasets"][k]["archive_sha256"] for k in keys}}
    if args.audit_only:
        (args.output / "data-audit.json").write_text(json.dumps({**meta, "audit": audits}, indent=1) + "\n", encoding="utf-8", newline="\n")
        return 0
    cases = {"schema_version": "triad-evidence-outcomes-cases-1", "cases": sink.cases}
    blob = json.dumps(cases, separators=(",", ":")).encode()
    (args.output / "cases.json").write_bytes(blob)
    (args.output / "truth.json").write_text(json.dumps({**meta, "cases_sha256": hashlib.sha256(blob).hexdigest(),
                                                       "truth": sink.truth}, indent=1) + "\n", encoding="utf-8", newline="\n")
    (args.output / "units.json").write_text(json.dumps({**meta, "audit": audits, "units": units}, indent=1) + "\n", encoding="utf-8", newline="\n")
    print(f"{len(sink.cases)} cases", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
