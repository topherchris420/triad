#!/usr/bin/env python3
"""Independent check of TRIAD's abstention certification (Milestone 9).

  python scripts/verify-certify.py --triad target/release/triad --random 60
      Generate random calibration studies, run `calibrate-abstention` and
      `certify-abstention` for both deployable methods, recompute every
      candidate from first principles here, and require agreement. Also checks
      that Hoeffding-Bentkus qualifies everything the legacy controller does.

  python scripts/verify-certify.py --study results/certify-v1/report.json
      Recompute rates, Clopper-Pearson bounds and every pre-registered verdict
      of the Milestone 9 report from its cell counts.

Standard library only. Written separately from src/certify.rs: the binomial
CDF uses lgamma, bounds use their own bisection, and sequence order is derived
here from SHA-256. Agreement shows the computation is faithful to the stated
method, not that the method's assumptions hold for any real data.
"""
import argparse
import hashlib
import json
import math
import random
import subprocess
import sys
import tempfile
from pathlib import Path

REL, ABS = 1e-9, 1e-12


def kl(p, q):
    if q <= 0.0:
        return 0.0 if p <= 0.0 else math.inf
    if q >= 1.0:
        return 0.0 if p >= 1.0 else math.inf
    out = 0.0
    if p > 0.0:
        out += p * math.log(p / q)
    if p < 1.0:
        out += (1.0 - p) * math.log((1.0 - p) / (1.0 - q))
    return max(out, 0.0)


def binom_cdf(k, n, p):
    if k < 0:
        return 0.0
    if k >= n or p <= 0.0:
        return 1.0
    if p >= 1.0:
        return 0.0
    logs = [math.lgamma(n + 1) - math.lgamma(i + 1) - math.lgamma(n - i + 1)
            + i * math.log(p) + (n - i) * math.log1p(-p) for i in range(k + 1)]
    top = max(logs)
    return min(1.0, math.exp(top) * math.fsum(math.exp(x - top) for x in logs))


def hb_p(mean, n, alpha):
    if alpha >= 1.0:
        return 0.0
    if alpha <= 0.0 or n == 0:
        return 1.0
    h = math.exp(-n * kl(min(mean, alpha), alpha))
    b = math.e * binom_cdf(math.ceil(n * mean), n, alpha)
    return min(h, b, 1.0)


def kl_lower(mean, n, delta):
    if n == 0 or mean <= 0.0:
        return 0.0
    budget = math.log(1.0 / delta) / n
    lo, hi = 0.0, mean
    while hi - lo > 1e-15:
        mid = (lo + hi) / 2
        if kl(mean, mid) > budget:
            lo = mid
        else:
            hi = mid
    return max(lo, mean - math.sqrt(budget / 2), 0.0)


def betting_p(xs, m, delta, upper):
    """Waudby-Smith and Ramdas predictable plug-in capital, fixed-n tuning."""
    n = len(xs)
    room = m if upper else 1.0 - m
    if n == 0 or room <= 0.0:
        return 1.0
    cap, scale = 0.5 / room, 2.0 * math.log(1.0 / delta) / n
    total = sq = logk = best = 0.0
    for t, x in enumerate(xs, start=1):
        var = (0.25 + sq) / t
        lam = min(math.sqrt(scale / var), cap)
        logk += math.log1p(lam * ((x - m) if upper else (m - x)))
        best = max(best, logk)
        total += x
        mu = (0.5 + total) / (t + 1)
        sq += (x - mu) ** 2
    return min(1.0, math.exp(-best))


def recompute(request, method):
    spec, seqs = request["spec"], request["sequences"]
    alpha, cmin, delta = spec["maximum_accepted_error_rate"], spec["minimum_coverage"], spec["delta"]
    ts = spec["candidate_thresholds"]
    K, n, F = len(ts), len(seqs), len(seqs[0]["frames"])
    per = delta / (2 * K)
    radius = math.sqrt(math.log(2 * K / delta) / (2 * n))
    order = sorted(range(n), key=lambda i: hashlib.sha256(seqs[i]["id"].encode()).digest())
    out = []
    for t in ts:
        per_seq = [(sum(f["score"] <= t for f in seqs[i]["frames"]),
                    sum(f["score"] <= t and f["incorrect"] for f in seqs[i]["frames"])) for i in order]
        acc = sum(a for a, _ in per_seq)
        bad = sum(b for _, b in per_seq)
        cov, joint = acc / (n * F), bad / (n * F)
        llow = max(cov - radius, 0.0)
        lup = min(1.0, min(1.0, joint + radius) / llow) if llow > 0 else 1.0
        row = {"threshold": t, "observed_coverage": cov,
               "legacy_qualifies": llow >= cmin and lup <= alpha,
               "legacy_lower": llow, "legacy_upper": lup}
        if method == "hoeffding-bentkus":
            row["coverage_lower_bound"] = max(kl_lower(cov, n, per), llow)
            row["risk_p_value"] = hb_p(joint - alpha * cov + alpha, n, alpha)
            row["coverage_certified"] = row["coverage_lower_bound"] >= cmin
            row["risk_certified"] = row["risk_p_value"] <= per or lup <= alpha
        elif method == "betting":
            zs = [min(1.0, max(0.0, b / F - alpha * (a / F) + alpha)) for a, b in per_seq]
            row["risk_p_value"] = 0.0 if alpha >= 1.0 else betting_p(zs, alpha, per, False)
            row["coverage_p_value"] = betting_p([a / F for a, _ in per_seq], cmin, per, True)
            row["coverage_certified"] = row["coverage_p_value"] <= per
            row["risk_certified"] = row["risk_p_value"] <= per
        row["qualifies"] = row.get("coverage_certified", False) and row.get("risk_certified", False)
        row["per_test"] = per
        out.append(row)
    return out


def near(a, b):
    return abs(a - b) <= max(ABS, REL * abs(b))


def random_request(rng, index):
    n = rng.randrange(5, 1500)
    frames = rng.randrange(1, 5)
    base = rng.uniform(0.0, 0.06)
    slope = rng.uniform(0.0, 0.5)
    k = rng.randrange(1, 7)
    ts = sorted({round(rng.uniform(0.05, 1.0), 3) for _ in range(k)})
    seqs = []
    for i in range(n):
        shared = rng.uniform(0.0, 2.0) if frames > 1 and rng.random() < 0.5 else 1.0
        fr = []
        for _ in range(frames):
            s = rng.random()
            fr.append({"score": s, "incorrect": rng.random() < min(1.0, (base + slope * s ** 3) * shared)})
        seqs.append({"id": f"study-{index}-sequence-{i}", "frames": fr})
    spec = {"method_id": f"verify-certify-{index}",
            "maximum_accepted_error_rate": rng.choice([0.02, 0.05, 0.1, 0.2, 1.0]),
            "minimum_coverage": rng.uniform(0.05, 0.9),
            "delta": rng.choice([0.01, 0.05, 0.1, 0.2]),
            "candidate_thresholds": ts}
    return {"spec": spec, "sequences": seqs}


def run_cli(triad, args, request):
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "request.json"
        path.write_text(json.dumps(request), encoding="utf-8")
        out = subprocess.run([triad, *args, "--input", str(path)], check=True, capture_output=True)
    return json.loads(out.stdout)


def check_random(triad, count, seed):
    rng = random.Random(seed)
    checked = boundary = ready = gains = 0
    for index in range(count):
        request = random_request(rng, index)
        legacy = run_cli(triad, ["calibrate-abstention"], request)
        for method in ("hoeffding-bentkus", "betting"):
            cert = run_cli(triad, ["certify-abstention", "--method", method], request)
            mine = recompute(request, method)
            assert len(mine) == len(cert["candidates"])
            for a, b in zip(cert["candidates"], mine):
                assert a["threshold"] == b["threshold"]
                assert near(a["observed_coverage"], b["observed_coverage"]), (index, a, b)
                for key in ("coverage_lower_bound", "risk_p_value", "coverage_p_value"):
                    if key in b:
                        assert near(a[key], b[key]), (index, method, key, a[key], b[key])
                for key in ("coverage_certified", "risk_certified"):
                    if a[key] != b[key]:
                        p = b.get("risk_p_value" if key == "risk_certified" else "coverage_p_value")
                        bound = b.get("coverage_lower_bound")
                        edge = (p is not None and near(p, b["per_test"])) or \
                               (bound is not None and near(bound, request["spec"]["minimum_coverage"]))
                        assert edge, (index, method, key, a, b)
                        boundary += 1
                checked += 1
            if method == "hoeffding-bentkus":
                for row, lc in zip(cert["candidates"], legacy["candidates"]):
                    assert not lc["qualifies"] or row["qualifies"], ("dominance", index)
                    assert row["coverage_lower_bound"] >= lc["coverage_lower_bound"], ("bound", index)
                ready += legacy["status"] == "ready"
                gains += cert["status"] == "ready" and legacy["status"] != "ready"
        for lc, b in zip(legacy["candidates"], recompute(request, "hoeffding-bentkus")):
            assert near(lc["coverage_lower_bound"], b["legacy_lower"]), (index, lc, b)
            assert near(lc["conditional_error_upper_bound"], b["legacy_upper"]), (index, lc, b)
    print(f"PASS: {checked} candidate certificates across {count} random studies match the "
          f"independent recomputation ({boundary} decisions at a floating-point boundary); "
          f"Hoeffding-Bentkus dominated legacy in all {count} (legacy ready {ready}, "
          f"certified only by Hoeffding-Bentkus {gains})")


def cp(k, n, level, upper):
    if upper and k >= n:
        return 1.0
    if not upper and k == 0:
        return 0.0
    tail = 1.0 - level
    lo, hi = 0.0, 1.0
    for _ in range(200):
        mid = (lo + hi) / 2
        low_side = binom_cdf(k, n, mid) > tail if upper else 1.0 - binom_cdf(k - 1, n, mid) < tail
        lo, hi = (mid, hi) if low_side else (lo, mid)
    return hi if upper else lo


def check_study(path):
    report = json.loads(Path(path).read_text(encoding="utf-8"))["report"]
    proto = report["protocol"]
    delta = proto["targets"]["delta"]
    cells = report["cells"]
    assert len(cells) == 2 * 2 * len(proto["sequence_counts"])
    valid = ("legacy-hoeffding-ratio", "hoeffding-bentkus", "betting")
    exceed, n80 = [], {}
    for c in cells:
        reps = c["replicates"]
        assert reps == proto["replicates_per_cell"]
        assert c["seed"] == proto["master_seed"] + 1000 * c["cell_index"]
        for name, m in c["methods"].items():
            assert m["false_certified"] <= m["certified"] <= reps
            assert m["false_certified"] <= m["false_risk"] + m["false_coverage"]
            assert max(m["false_risk"], m["false_coverage"]) <= m["false_certified"]
            assert m["certification_rate"] == m["certified"] / reps
            assert m["false_certification_rate"] == m["false_certified"] / reps
            lo = cp(m["false_certified"], reps, 0.99, False)
            hi = cp(m["false_certified"], reps, 0.99, True)
            assert abs(lo - m["false_certification_lower_99"]) <= 2e-6, (c["cell_index"], name)
            assert abs(hi - m["false_certification_upper_99"]) <= 2e-6, (c["cell_index"], name)
            assert 0.0 <= m["mean_selected_true_coverage"] <= m["certification_rate"] + 1e-6
            if c["error_model"] == "null":
                assert m["false_certified"] == m["certified"], "every null certification is false"
            if name in valid and lo > delta:
                exceed.append({"cell_index": c["cell_index"], "method": name})
            if c["error_model"] == "graded" and c["regime"] == "dependent_frames" \
                    and m["certification_rate"] >= 0.8 and name not in n80:
                n80[name] = c["sequences"]
    h = report["hypotheses"]
    assert h["H1_validity"]["supported"] == (not exceed)
    assert h["H1_validity"]["exceedances"] == exceed
    violations = sum(c["dominance_violations"] for c in cells)
    assert h["H2_dominance"]["violating_datasets"] == violations
    assert h["H2_dominance"]["supported"] == (violations == 0)
    for key, name in (("H3_primary_efficiency", "hoeffding-bentkus"), ("H4_secondary_efficiency", "betting")):
        new, base = n80.get(name), n80.get("legacy-hoeffding-ratio")
        expect = (new is not None) and (2 * new <= base if base is not None else new <= 12500)
        assert h[key]["n80"] == new and h[key]["legacy_n80"] == base, key
        assert h[key]["supported"] == expect, key
    naive = [c for c in cells if c["error_model"] == "null" and c["regime"] == "dependent_frames"]
    exceeds = any(cp(c["methods"]["naive-frame-binomial"]["false_certified"], c["replicates"], 0.99, False) > delta
                  for c in naive)
    assert h["E1_exploratory"]["naive_exceeds_delta"] == exceeds
    print(f"PASS: {len(cells)} Milestone 9 cells; counts, Clopper-Pearson bounds and all "
          f"pre-registered verdicts recomputed independently "
          f"(H1 {h['H1_validity']['supported']}, H2 {h['H2_dominance']['supported']}, "
          f"H3 {h['H3_primary_efficiency']['supported']}, H4 {h['H4_secondary_efficiency']['supported']}, "
          f"E1 naive exceeds delta {exceeds})")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--triad", help="path to the built triad binary")
    ap.add_argument("--random", type=int, default=0, help="number of random studies to cross-check")
    ap.add_argument("--seed", type=int, default=20261015)
    ap.add_argument("--study", help="Milestone 9 report.json to re-derive")
    args = ap.parse_args()
    if not args.random and not args.study:
        ap.error("nothing to check: pass --random N with --triad, and/or --study REPORT")
    if args.random:
        if not args.triad:
            ap.error("--random requires --triad")
        check_random(args.triad, args.random, args.seed)
    if args.study:
        check_study(args.study)


if __name__ == "__main__":
    sys.exit(main())
