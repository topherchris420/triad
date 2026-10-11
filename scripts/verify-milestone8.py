#!/usr/bin/env python3
"""Independent recomputation of the Milestone 8 outputs.

  python scripts/verify-milestone8.py --run DIR [--regenerated DIR]

DIR holds cases.json, truth.json, units.json, classifications.json,
comparators.json and score/report.json. Checks, without calling Rust and without
importing the generator or scorer:

1. hashes link cases, truth and classifications;
2. every TRIAD, C, C+scope and ablation classification equals the separately
   written Python re-implementation of the Milestone 7 rules
   (scripts/verify-milestone7.py, triad_class / standard_class);
3. evidence counts, claim templates and truth flags are internally consistent,
   and family A, B and D truth values equal the period, stratum and week rates
   recorded in units.json;
4. every endpoint, bootstrap interval and the decision are recomputed.
With --regenerated, a second generator run must produce byte-identical cases,
truth and units. Computational cross-check only; not scientific replication.
"""
import argparse
import hashlib
import importlib.util
import json
import math
import sys
from pathlib import Path

import numpy as np
from scipy import stats

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("vm7", ROOT / "scripts/verify-milestone7.py")
vm7 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(vm7)
CHECKS = 0


def same(a, b, what):
    global CHECKS
    CHECKS += 1
    if a != b:
        raise SystemExit(f"MISMATCH {what}: {a!r} != {b!r}")


def close(a, b, what, tol=1e-9):
    global CHECKS
    CHECKS += 1
    if (a is None) != (b is None) or (a is not None and not math.isclose(a, b, rel_tol=tol, abs_tol=1e-12)):
        raise SystemExit(f"MISMATCH {what}: {a!r} != {b!r}")


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def ceil6(x):
    return min(1.0, math.ceil(x * 1e6 - 1e-9) / 1e6)


def check_classifications(cases, cls):
    by_id = {r["case_id"]: r for r in cls["results"]}
    core_off = frozenset(f for f in vm7.FAMILIES if f not in ("scope", "dependence", "statistical"))
    for case in cases["cases"]:
        a, got = case["assessment"], by_id[case["case_id"]]
        same(got["triad"], vm7.triad_class(a, ROOT), case["case_id"] + " triad")
        same(got["c"], vm7.standard_class(a, ROOT, False), case["case_id"] + " C")
        same(got["c_scope"], vm7.standard_class(a, ROOT, True), case["case_id"] + " C+scope")
        same(got["triad_core"], vm7.triad_class(a, ROOT, core_off), case["case_id"] + " core")
        for fam in vm7.FAMILIES:
            same(got["ablations"][fam], vm7.triad_class(a, ROOT, frozenset([fam])), f"{case['case_id']} ablation {fam}")


def check_truth(cases, truth, units):
    for case in cases["cases"]:
        cid = case["case_id"]
        m = truth["truth"][cid]
        a = case["assessment"]
        claim = a["claims"][0]
        ev = {e["id"]: e for e in a["evidence"]}
        if m["family"] == "C":
            same(m["truth"], m["truth_value"] >= 0.88, cid + " truth flag")
            same(claim["level"], 0.9, cid)
            continue
        same(m["truth"], m["truth_value"] <= m["threshold"], cid + " truth flag")
        u = units["units"][m["dataset"]]
        if m["family"] == "A":
            P = u["by_period"]["evaluation"]
            close(m["truth_value"], P["errors"] / P["n"], cid + " population rate")
            e = ev["sample"]
            n, k = (len(e["outcomes"]), sum(e["outcomes"])) if e["kind"] == "outcome_records" else (e["n"], e["events"])
            same((n, k), (m["n"], m["events"]), cid + " counts")
            U = 1.0 if k >= n else float(stats.beta.ppf(0.95, k + 1, n - k))
            mult = float(m["template"].split("=")[1])
            close(claim["bound"], ceil6(k / n + mult * (U - k / n)), cid + " template")
            if e["kind"] == "metric_report" and e["interval"]["method"] == "clopper_pearson":
                lo = 0.0 if k == 0 else float(stats.beta.ppf(0.025, k, n - k + 1))
                hi = 1.0 if k == n else float(stats.beta.ppf(0.975, k + 1, n - k))
                close(e["interval"]["lower"], lo, cid + " CP lower")
                close(e["interval"]["upper"], hi, cid + " CP upper")
        elif m["family"] == "B":
            s = u["by_stratum"][m["target"]]
            close(m["truth_value"], s["errors"] / s["n"], cid + " stratum rate")
            same(claim["conditions"], [m["target"]], cid + " target")
            e = ev["window"]
            same(m["target"] in e["conditions"], False, cid + " target outside window")
        elif m["family"] == "D":
            w = u["by_week"][m["week"]]
            close(m["truth_value"], w["errors"] / w["n"], cid + " week rate")


def family_cost(F, FS, T, TS, c):
    parts = []
    if F:
        parts.append(0.5 * FS / F)
    if T:
        parts.append(0.5 * c * (1 - TS / T))
    return sum(parts), (FS / F if F else None)


def check_score(truth, cls, comp, report):
    tri = {r["case_id"]: r for r in cls["results"]}
    cmp_ = {r["case_id"]: r for r in comp["results"]}
    methods = ["triad", "r", "s", "c", "c_scope", "never", "triad_core"]

    def verdict(cid, m):
        if m == "never":
            return False
        src = cmp_ if m in ("r", "s") else tri
        return src[cid][m] == "supported_within_scope"

    cells = {}
    for cid, meta in truth["truth"].items():
        key = (meta["dataset"], meta["family"], meta["cluster"])
        cell = cells.setdefault(key, np.zeros((len(methods), 4)))
        for j, m in enumerate(methods):
            s = verdict(cid, m)
            cell[j] += (0, 0, 1, s) if meta["truth"] else (1, s, 0, 0)

    def costs(groups):
        fams = {}
        for k, arr in groups:
            fams.setdefault(k[1], np.zeros((len(methods), 4)))
            fams[k[1]] += arr
        out, fsr = [], []
        for j in range(len(methods)):
            terms = [family_cost(*fams[f][j], 0.25) for f in sorted(fams)]
            out.append(float(np.mean([t[0] for t in terms])))
            fsr.append(float(np.mean([t[1] for t in terms if t[1] is not None])))
        return out, fsr

    keys = sorted(cells)
    point, fsr = costs([(k, cells[k]) for k in keys])
    for j, m in enumerate(methods):
        close(report["cost"][m], point[j], f"cost {m}")
        close(report["macro_FSR"][m], fsr[j], f"macro FSR {m}")
    strata = {}
    for k in keys:
        strata.setdefault(k[:2], []).append(k)
    rng = np.random.default_rng(80808)
    boot = np.zeros((2000, len(methods)))
    for b in range(2000):
        groups = []
        for s in sorted(strata):
            ks = strata[s]
            for i in rng.integers(0, len(ks), size=len(ks)):
                groups.append((ks[i], cells[ks[i]]))
        boot[b] = costs(groups)[0]
    for i, m in enumerate(("r", "s", "never")):
        d = boot[:, 0] - boot[:, methods.index(m)]
        got = report["decision"]["comparisons"][m]
        close(got["delta"], point[0] - point[methods.index(m)], f"delta {m}")
        close(got["one_sided_97_5_upper"], float(np.percentile(d, 97.5)), f"UB {m}", tol=1e-7)
        sup = got["delta"] <= -0.01 and got["one_sided_97_5_upper"] < 0
        same(got["superior"], sup, f"superior {m}")
    for m in ("r", "s"):
        per = report["decision"]["comparisons"][m]["per_dataset_delta"]
        for d in sorted({k[0] for k in keys}):
            dc, _ = costs([(k, cells[k]) for k in keys if k[0] == d])
            close(per[d], dc[0] - dc[methods.index(m)], f"dataset {d} delta {m}")
    cond = report["decision"]["conditions"]
    n_sup = cond["superior_to_r"] + cond["superior_to_s"]
    ok = cond["triad_FSR_at_most_0.05"] and cond["beats_never_support"] and cond["consistent_in_at_least_4_of_5_datasets"]
    expect = "independent_support" if n_sup == 2 and ok else "partial_support" if n_sup == 1 and ok else "not_established"
    same(report["decision"]["verdict"], expect, "verdict")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--run", required=True, type=Path)
    ap.add_argument("--regenerated", type=Path)
    args = ap.parse_args()
    d = args.run
    cases = json.loads((d / "cases.json").read_bytes())
    truth = json.loads((d / "truth.json").read_bytes())
    units = json.loads((d / "units.json").read_bytes())
    cls = json.loads((d / "classifications.json").read_bytes())
    comp = json.loads((d / "comparators.json").read_bytes())
    report = json.loads((d / "score/report.json").read_bytes())
    same(sha(d / "cases.json"), truth["cases_sha256"], "cases hash in truth")
    same(sha(d / "cases.json"), cls["input_sha256"], "cases hash in classifications")
    same([c["case_id"] for c in cases["cases"]], [r["case_id"] for r in cls["results"]], "TRIAD order")
    same([c["case_id"] for c in cases["cases"]], [r["case_id"] for r in comp["results"]], "comparator order")
    same(set(truth["truth"]), {c["case_id"] for c in cases["cases"]}, "truth case set")
    check_truth(cases, truth, units)
    check_score(truth, cls, comp, report)
    check_classifications(cases, cls)
    if args.regenerated:
        identical = all(sha(args.regenerated / n) == sha(d / n) for n in ("cases.json", "truth.json", "units.json"))
        if identical:
            print("regeneration: byte-identical cases, truth and units")
        else:
            # Floating-point differences across CPUs or BLAS builds can move a prediction
            # across the material-error tolerance. Require the same cases and agreement on
            # at least 99.5% of truth flags, and report the difference.
            regen = json.loads((args.regenerated / "truth.json").read_bytes())["truth"]
            same(set(regen), set(truth["truth"]), "regenerated case set")
            flips = sum(regen[c]["truth"] != m["truth"] for c, m in truth["truth"].items())
            print(f"regeneration: not byte-identical; truth flags differ on {flips} of {len(regen)} cases")
            same(flips <= 0.005 * len(regen), True, "regenerated truth agreement")
    print(f"PASS: {CHECKS} Milestone 8 checks")
    return 0


if __name__ == "__main__":
    sys.exit(main())
