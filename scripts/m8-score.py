#!/usr/bin/env python3
"""Milestone 8 scorer: endpoints and pre-registered decision (protocol.json).

  python scripts/m8-score.py --truth truth.json --triad classifications.json \
      --comparators comparators.json --output NEW_DIR

Only this script reads both truth and classifications. Writes NEW_DIR/report.json
and NEW_DIR/report.md. Deterministic (bootstrap seed 80808).
"""
import argparse
import hashlib
import json
import sys
from pathlib import Path

import numpy as np

SEED = 80808
BOOT = 2000
C_MAIN = 0.25
PRACTICAL = 0.01
MIN_CLAIMS = 20
FAMILIES = ["A", "B", "C", "D"]
STRONG = ["r", "s"]
SUPPORTED = "supported_within_scope"


def load(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def collect(truth, triad, comparators):
    tri = {r["case_id"]: r for r in triad["results"]}
    comp = {r["case_id"]: r for r in comparators["results"]}
    if set(tri) != set(truth["truth"]) or set(comp) != set(truth["truth"]):
        raise SystemExit("case sets of truth, TRIAD and comparators differ")
    rows = []
    for cid, meta in truth["truth"].items():
        t, c = tri[cid], comp[cid]
        verdicts = {"triad": t["triad"], "c": t["c"], "c_scope": t["c_scope"], "triad_core": t["triad_core"],
                    "r": c["r"], "s": c["s"], "never": "insufficient_evidence"}
        for fam, v in t["ablations"].items():
            verdicts[f"triad_without_{fam}"] = v
        rows.append({"case_id": cid, **meta, "verdicts": verdicts, "triad_blocking": t["triad_blocking"]})
    return rows


def tallies(rows, methods, truth_key="truth"):
    """Per (dataset, family, cluster): counts [false, false_supported, true, true_supported] per method."""
    out = {}
    for r in rows:
        key = (r["dataset"], r["family"], r["cluster"])
        cell = out.setdefault(key, {m: np.zeros(4) for m in methods})
        true = bool(r[truth_key])
        for m in methods:
            s = r["verdicts"][m] == SUPPORTED
            cell[m] += (0, 0, 1, s) if true else (1, s, 0, 0)
    return out


def family_terms(sums, c):
    """sums: family -> [F, FS, T, TS]. Returns cost, per-family detail."""
    terms, detail = [], {}
    for fam, (F, FS, T, TS) in sums.items():
        parts = []
        fsr = FS / F if F > 0 else None
        tsr = TS / T if T > 0 else None
        if fsr is not None:
            parts.append(0.5 * fsr)
        if tsr is not None:
            parts.append(0.5 * c * (1 - tsr))
        terms.append(sum(parts))
        detail[fam] = {"false": int(F), "false_supported": int(FS), "true": int(T), "true_supported": int(TS),
                       "FSR": fsr, "TSR": tsr, "flag_fewer_than_20": bool(F < MIN_CLAIMS or T < MIN_CLAIMS)}
    return (float(np.mean(terms)) if terms else None), detail


def aggregate(cells, method, keys, by="family"):
    sums = {}
    for k in keys:
        dataset, fam, _ = k
        g = fam if by == "family" else (dataset, fam)
        sums.setdefault(g, np.zeros(4))
        sums[g] += cells[k][method]
    return sums


def cost_of(cells, method, keys, c=C_MAIN):
    return family_terms(aggregate(cells, method, keys), c)


def macro_fsr(detail):
    v = [d["FSR"] for d in detail.values() if d["FSR"] is not None]
    return float(np.mean(v)) if v else None


def macro_tsr(detail):
    v = [d["TSR"] for d in detail.values() if d["TSR"] is not None]
    return float(np.mean(v)) if v else None


def bootstrap(cells, methods, c=C_MAIN):
    strata = {}
    for k in sorted(cells):
        strata.setdefault((k[0], k[1]), []).append(k)
    rng = np.random.default_rng(SEED)
    # Pre-stack arrays: per stratum, (clusters x methods x 4)
    stacks = {s: np.stack([[cells[k][m] for m in methods] for k in ks]) for s, ks in strata.items()}
    costs = np.zeros((BOOT, len(methods)))
    fsrs = np.zeros((BOOT, len(methods)))
    for b in range(BOOT):
        fam_sums = {}
        for (dataset, fam), arr in stacks.items():
            pick = rng.integers(0, len(arr), size=len(arr))
            fam_sums.setdefault(fam, np.zeros((len(methods), 4)))
            fam_sums[fam] += arr[pick].sum(0)
        for j, _ in enumerate(methods):
            cost, detail = family_terms({f: s[j] for f, s in fam_sums.items()}, c)
            costs[b, j] = cost
            fsrs[b, j] = macro_fsr(detail)
    return costs, fsrs


def interval(x, lo=2.5, hi=97.5):
    return [float(np.percentile(x, lo)), float(np.percentile(x, hi))]


def decide(point, boot, methods, per_dataset, triad_fsr, fsr_boot):
    j = {m: i for i, m in enumerate(methods)}
    out = {"comparisons": {}}
    superior = {}
    for m in STRONG + ["never"]:
        d = boot[:, j["triad"]] - boot[:, j[m]]
        delta = point["triad"] - point[m]
        ub = float(np.percentile(d, 97.5))
        sup = bool(delta <= -PRACTICAL and ub < 0)
        ds = {k: v["triad"] - v[m] for k, v in per_dataset.items()}
        out["comparisons"][m] = {
            "delta": delta, "one_sided_97_5_upper": ub, "two_sided_95": interval(d),
            "superior": sup, "datasets_with_delta_below_0": sum(1 for v in ds.values() if v < 0),
            "per_dataset_delta": ds}
        superior[m] = sup
    n_sup = sum(superior[m] for m in STRONG)
    consistent = all(out["comparisons"][m]["datasets_with_delta_below_0"] >= 4 for m in STRONG)
    beats_never = out["comparisons"]["never"]["delta"] < 0 and out["comparisons"]["never"]["one_sided_97_5_upper"] < 0
    valid = triad_fsr is not None and triad_fsr <= 0.05
    out["conditions"] = {"superior_to_r": superior["r"], "superior_to_s": superior["s"],
                         "triad_family_averaged_FSR": triad_fsr, "triad_FSR_bootstrap_95": interval(fsr_boot),
                         "triad_FSR_at_most_0.05": valid, "beats_never_support": bool(beats_never),
                         "consistent_in_at_least_4_of_5_datasets": consistent}
    if n_sup == 2 and valid and beats_never and consistent:
        verdict = "independent_support"
    elif n_sup == 1 and valid and beats_never and consistent:
        verdict = "partial_support"
    else:
        verdict = "not_established"
        best = min(STRONG, key=lambda m: point[m])
        lo, hi = out["comparisons"][best]["two_sided_95"]
        out["versus_better_strong_comparator"] = {
            "comparator": best,
            "classification": "equivalent" if (lo >= -PRACTICAL and hi <= PRACTICAL) else
            "inferior" if lo > 0 else "superior" if hi < 0 else "inconclusive"}
    out["verdict"] = verdict
    return out


def summarise(rows, methods, truth_key="truth", c=C_MAIN):
    cells = tallies(rows, methods, truth_key)
    keys = sorted(cells)
    point, details = {}, {}
    for m in methods:
        point[m], details[m] = cost_of(cells, m, keys, c)
    return cells, keys, point, details


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--truth", required=True)
    ap.add_argument("--triad", required=True)
    ap.add_argument("--comparators", required=True)
    ap.add_argument("--output", required=True, type=Path)
    args = ap.parse_args()
    truth, triad, comp = load(args.truth), load(args.triad), load(args.comparators)
    if triad["input_sha256"] != truth["cases_sha256"]:
        raise SystemExit("TRIAD classifications were not computed from the cases that truth.json seals")
    rows = collect(truth, triad, comp)
    main_methods = ["triad", "r", "s", "c", "c_scope", "never", "triad_core"]
    ablation_methods = sorted(m for m in rows[0]["verdicts"] if m.startswith("triad_without_"))
    methods = main_methods + ablation_methods
    cells, keys, point, details = summarise(rows, methods)
    boot, fsr_boot = bootstrap(cells, main_methods)
    datasets = sorted({k[0] for k in keys})
    per_dataset = {}
    for d in datasets:
        dk = [k for k in keys if k[0] == d]
        per_dataset[d] = {m: cost_of(cells, m, dk)[0] for m in main_methods}
    decision = decide(point, boot, main_methods, per_dataset, macro_fsr(details["triad"]), fsr_boot[:, 0])

    # Secondary analyses
    j = {m: i for i, m in enumerate(main_methods)}
    secondary = {}
    secondary["cost_95"] = {m: interval(boot[:, j[m]]) for m in main_methods}
    secondary["replication_versus_c"] = {m: {"delta": point["triad"] - point[m],
                                            "two_sided_95": interval(boot[:, j["triad"]] - boot[:, j[m]])}
                                        for m in ("c", "c_scope")}
    secondary["per_dataset_family"] = {}
    for m in main_methods:
        secondary["per_dataset_family"][m] = {f"{d}/{f}": v for (d, f), v in
                                              family_terms(aggregate(cells, m, keys, by="dataset"), C_MAIN)[1].items()}
    secondary["cost_sensitivity_c"] = {str(c): {m: summarise(rows, main_methods, c=c)[2][m] for m in main_methods}
                                       for c in (0.1, 0.5)}
    tol = {}
    for t in (0.01, 0.03):
        alt = []
        for r in rows:
            r2 = dict(r)
            if r["family"] == "C":
                r2["truth"] = r["truth_value"] >= 0.90 - t
            alt.append(r2)
        tol[str(t)] = summarise(alt, main_methods)[2]
    secondary["coverage_tolerance_sensitivity"] = tol
    pooled = {}
    for m in main_methods:
        F = sum(1 for r in rows if not r["truth"])
        FS = sum(1 for r in rows if not r["truth"] and r["verdicts"][m] == SUPPORTED)
        T = len(rows) - F
        TS = sum(1 for r in rows if r["truth"] and r["verdicts"][m] == SUPPORTED)
        pooled[m] = {"cost_per_claim": (FS + C_MAIN * (T - TS)) / len(rows), "false": F, "false_supported": FS,
                     "true": T, "true_supported": TS}
    secondary["empirical_prevalence_pooled"] = pooled
    by_ds = {}
    for factor in ("design", "style"):
        groups = sorted({r.get(factor) for r in rows if r["family"] == "A"})
        by_ds[factor] = {}
        for g in groups:
            sub = [r for r in rows if r["family"] == "A" and r.get(factor) == g]
            by_ds[factor][g] = {m: family_terms(aggregate(tallies(sub, [m]), m, sorted(tallies(sub, [m]))), C_MAIN)[1]["A"]
                                for m in main_methods}
    secondary["family_A_by_design_and_style"] = by_ds
    contra = [r for r in rows if r["verdicts"]["triad"] == "contradicted"]
    secondary["triad_contradicted_precision"] = {"contradicted": len(contra),
                                                 "false_among_them": sum(1 for r in contra if not r["truth"])}
    secondary["ablation_cost"] = {m: point[m] for m in ablation_methods}
    agree = lambda a, b: float(np.mean([(r["verdicts"][a] == SUPPORTED) == (r["verdicts"][b] == SUPPORTED) for r in rows]))
    secondary["distinctness"] = {"support_agreement_triad_c_scope": agree("triad", "c_scope"),
                                 "support_agreement_triad_core": agree("triad", "triad_core"),
                                 "support_agreement_triad_r": agree("triad", "r"),
                                 "support_agreement_triad_s": agree("triad", "s"),
                                 "cost_triad_core": point["triad_core"]}
    best = min(STRONG, key=lambda m: point[m])
    disc = {}
    for r in rows:
        a, b = r["verdicts"]["triad"] == SUPPORTED, r["verdicts"][best] == SUPPORTED
        if a == b:
            continue
        fams = sorted({x["family"] for x in r["triad_blocking"]}) or ["none"]
        key = f"{'triad_supports' if a else best + '_supports'} | {'true' if r['truth'] else 'false'} | {'+'.join(fams)} | {r['family']}"
        disc[key] = disc.get(key, 0) + 1
    secondary["discordance_with_better_strong_comparator"] = {"comparator": best, "counts": dict(sorted(disc.items()))}

    report = {
        "schema_version": "triad-m8-score-1",
        "protocol_version": truth["protocol_version"],
        "inputs_sha256": {k: hashlib.sha256(Path(getattr(args, k)).read_bytes()).hexdigest() for k in ("truth", "triad", "comparators")},
        "cases": len(rows),
        "claims_by_family": {f: {"true": sum(1 for r in rows if r["family"] == f and r["truth"]),
                                 "false": sum(1 for r in rows if r["family"] == f and not r["truth"])} for f in FAMILIES},
        "cost": point,
        "family_detail": {m: details[m] for m in main_methods},
        "macro_FSR": {m: macro_fsr(details[m]) for m in main_methods},
        "macro_TSR": {m: macro_tsr(details[m]) for m in main_methods},
        "per_dataset_cost": per_dataset,
        "decision": decision,
        "secondary": secondary,
        "bootstrap": {"replicates": BOOT, "seed": SEED, "strata": "dataset x family", "unit": "resampling cluster"},
    }
    args.output.mkdir(parents=False, exist_ok=False)
    (args.output / "report.json").write_text(json.dumps(report, indent=1) + "\n", encoding="utf-8", newline="\n")
    (args.output / "report.md").write_text(markdown(report), encoding="utf-8", newline="\n")
    # UTF-8 regardless of console encoding (the summary contains Greek letters; cp1252 on Windows cannot encode them).
    sys.stdout.buffer.write(markdown(report).encode("utf-8"))
    sys.stdout.flush()
    return 0


def fmt(x, p=4):
    return "—" if x is None else f"{x:.{p}f}"


def markdown(r):
    m = ["triad", "r", "s", "c", "c_scope", "never", "triad_core"]
    lines = [f"# Milestone 8 score ({r['protocol_version']})", "",
             f"Cases: {r['cases']}. Verdict: **{r['decision']['verdict']}**.", "",
             "| Method | Cost | 95% | macro FSR | macro TSR |", "|---|---:|---|---:|---:|"]
    for k in m:
        ci = r["secondary"]["cost_95"][k]
        lines.append(f"| {k} | {fmt(r['cost'][k])} | [{fmt(ci[0])}, {fmt(ci[1])}] | {fmt(r['macro_FSR'][k], 3)} | {fmt(r['macro_TSR'][k], 3)} |")
    lines += ["", "| Comparison | Δ cost (TRIAD − m) | one-sided 97.5% UB | superior | datasets Δ<0 |", "|---|---:|---:|---|---:|"]
    for k, v in r["decision"]["comparisons"].items():
        lines.append(f"| {k} | {fmt(v['delta'])} | {fmt(v['one_sided_97_5_upper'])} | {v['superior']} | {v['datasets_with_delta_below_0']} |")
    lines += ["", "| Family | Method | false | FS | FSR | true | TS | TSR |", "|---|---|---:|---:|---:|---:|---:|---:|"]
    for k in m:
        for f, d in sorted(r["family_detail"][k].items()):
            lines.append(f"| {f} | {k} | {d['false']} | {d['false_supported']} | {fmt(d['FSR'], 3)} | {d['true']} | {d['true_supported']} | {fmt(d['TSR'], 3)} |")
    lines += ["", "Conditions: " + json.dumps(r["decision"]["conditions"]), ""]
    return "\n".join(lines) + "\n"


if __name__ == "__main__":
    sys.exit(main())
