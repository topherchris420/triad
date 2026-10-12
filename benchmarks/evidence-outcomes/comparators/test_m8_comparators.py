"""Tests for m8_comparators on small synthetic assessments (no real data)."""

import copy
import json
import os
import subprocess
import sys

import numpy as np
import pytest

import m8_comparators as m

HERE = os.path.dirname(os.path.abspath(__file__))
SUP, INS, CON, NA = m.SUPPORTED, m.INSUFFICIENT, m.CONTRADICTED, m.NOT_APPLICABLE
POP = "model-x/test"


# ---------------------------------------------------------------- builders

def assessment(claim, evidence, aid="a1"):
    claim = dict(claim)
    claim.setdefault("evidence", [e["id"] for e in evidence])
    return {"schema_version": "triad-evidence-1", "assessment_id": aid,
            "evidence": evidence, "claims": [claim]}


def metric_claim(bound, conditions=("2014-09",), comparator="at_most", population=POP):
    return {"type": "metric_bound", "id": "claim", "statement": "s",
            "metric": "material_error_rate", "population": population,
            "conditions": list(conditions), "comparator": comparator, "bound": bound,
            "confidence": 0.95, "claimed_value": None, "tolerance": 0.0}


def report(n, events, dependence="independent", interval=None, design_effect=None,
           conditions=("2014-09",), eid="rep", population=POP, unit="hour"):
    return {"kind": "metric_report", "id": eid, "metric": "material_error_rate",
            "population": population, "conditions": list(conditions), "unit": unit,
            "n": n, "events": events, "value": None, "dependence": dependence,
            "design_effect": design_effect, "source": "synthetic", "interval": interval}


def records(outcomes, dependence="independent", design_effect=None,
            conditions=("2014-09",), eid="rec", population=POP):
    return {"kind": "outcome_records", "id": eid, "metric": "material_error_rate",
            "population": population, "conditions": list(conditions), "unit": "hour",
            "dependence": dependence, "outcomes": [bool(v) for v in outcomes],
            "design_effect": design_effect}


def iv(method, lower, upper, level=0.95):
    return {"method": method, "level": level, "lower": lower, "upper": upper}


def iid_outcomes(n, p, seed=0):
    rng = np.random.default_rng(seed)
    return (rng.random(n) < p).tolist()


def runs_outcomes(n=2000, run=50, nruns=4, seed=0):
    """Errors arrive in long contiguous runs: strong serial dependence."""
    rng = np.random.default_rng(seed)
    x = np.zeros(n, dtype=bool)
    for s in rng.choice(n // run, nruns, replace=False):
        x[s * run:(s + 1) * run] = True
    return x.tolist()


def both(a):
    return m.review_r(a), m.review_s(a)


# ---------------------------------------------------------------- metric_bound

def test_independent_report_supported_insufficient_contradicted():
    ev = [report(1000, 50)]  # CP one-sided 95% upper ~ 0.0629, lower ~ 0.0393
    assert both(assessment(metric_claim(0.07), ev)) == (SUP, SUP)
    assert both(assessment(metric_claim(0.055), ev)) == (INS, INS)
    assert both(assessment(metric_claim(0.03), ev)) == (CON, CON)


def test_at_least_comparator():
    ev = [report(1000, 50)]
    assert both(assessment(metric_claim(0.03, comparator="at_least"), ev)) == (SUP, SUP)
    assert both(assessment(metric_claim(0.08, comparator="at_least"), ev)) == (CON, CON)
    assert both(assessment(metric_claim(0.05, comparator="equals"), ev)) == (INS, INS)


def test_zero_events_and_tiny_n():
    assert both(assessment(metric_claim(0.01), [report(1000, 0)])) == (SUP, SUP)
    # n tiny: R's checklist needs n >= 30; S's exact bound 1-0.05**(1/5) ~ 0.451
    a = assessment(metric_claim(0.5), [report(5, 0)])
    assert both(a) == (INS, SUP)
    assert both(assessment(metric_claim(0.4), [report(5, 0)])) == (INS, INS)


def test_null_interval_independent_uses_counts():
    ev = [report(400, 8, interval=None)]
    assert both(assessment(metric_claim(0.04), ev)) == (SUP, SUP)


def test_iid_interval_on_temporal_data_is_not_trusted():
    naive = iv("normal_iid", 0.0365, 0.0635)
    ev = [report(1000, 50, dependence="temporal", interval=naive)]
    # bound at the naive upper limit: neither endorses
    assert both(assessment(metric_claim(0.064), ev)) == (INS, INS)
    # very generous bound: S's sqrt(n) fallback still endorses, R rejects the item
    assert both(assessment(metric_claim(0.3), ev)) == (INS, SUP)


def test_block_bootstrap_interval_for_temporal():
    ev = [report(1000, 50, dependence="temporal", interval=iv("block_bootstrap", 0.03, 0.075))]
    assert both(assessment(metric_claim(0.08), ev)) == (SUP, SUP)
    assert both(assessment(metric_claim(0.07), ev)) == (INS, INS)
    assert both(assessment(metric_claim(0.02), ev)) == (CON, CON)


def test_interval_level_handling():
    # level 0.80 two-sided is only 90% one-sided: R refuses, S rescales upward
    ev = [report(1000, 50, dependence="temporal", interval=iv("block_bootstrap", 0.04, 0.06, 0.8))]
    assert m.review_r(assessment(metric_claim(0.2), ev)) == INS
    assert m.review_s(assessment(metric_claim(0.2), ev)) == SUP
    assert m.review_s(assessment(metric_claim(0.061), ev)) == INS


def test_cluster_bootstrap_for_clustered_and_method_mismatch():
    ev = [report(1000, 50, dependence="clustered", interval=iv("cluster_bootstrap", 0.03, 0.075))]
    assert both(assessment(metric_claim(0.08), ev)) == (SUP, SUP)
    ev2 = [report(1000, 50, dependence="clustered", interval=iv("block_bootstrap", 0.03, 0.075))]
    assert both(assessment(metric_claim(0.08), ev2)) == (INS, SUP)


def test_design_effect_report():
    ev = [report(1000, 50, dependence="clustered", design_effect=4.0)]
    ub = m.cp_upper(12.5, 250, 0.05)
    assert 0.07 < ub < 0.085
    assert both(assessment(metric_claim(0.086), ev)) == (SUP, SUP)
    assert both(assessment(metric_claim(0.065), ev)) == (INS, INS)
    no_de = [report(1000, 50, dependence="clustered")]
    assert m.review_r(assessment(metric_claim(0.086), no_de)) == INS
    bad_de = [report(1000, 50, dependence="clustered", design_effect=0.5)]
    assert m.review_r(assessment(metric_claim(0.086), bad_de)) == INS


def test_unknown_dependence():
    ev = [report(1000, 50, dependence="unknown")]
    assert both(assessment(metric_claim(0.3), ev)) == (INS, SUP)
    assert both(assessment(metric_claim(0.07), ev)) == (INS, INS)
    ev2 = [report(1000, 50, dependence="unknown", design_effect=2.0)]
    assert both(assessment(metric_claim(0.075), ev2)) == (SUP, SUP)


def test_outcome_records_independent():
    out = iid_outcomes(800, 0.05, seed=1)
    k = sum(out)
    ub = m.cp_upper(k, 800, 0.05)
    ev = [records(out)]
    assert both(assessment(metric_claim(ub + 0.002), ev)) == (SUP, SUP)
    assert both(assessment(metric_claim(ub - 0.005), ev)) == (INS, INS)


def test_outcome_records_temporal_runs_widen_bounds():
    out = runs_outcomes()
    ev = [records(out, dependence="temporal")]
    iid_ub = m.cp_upper(sum(out), len(out), 0.05)
    # iid bound would endorse this; dependence-aware reviewers do not
    assert both(assessment(metric_claim(iid_ub + 0.005), ev)) == (INS, INS)
    assert both(assessment(metric_claim(0.6), ev)) == (SUP, SUP)
    # the same data labelled independent: both use the exact iid bound
    ev_ind = [records(out, dependence="independent")]
    assert both(assessment(metric_claim(iid_ub + 0.005), ev_ind)) == (SUP, SUP)


def test_outcome_records_clustered_and_unknown():
    out = iid_outcomes(1200, 0.05, seed=3)
    for dep in ("clustered", "unknown"):
        ev = [records(out, dependence=dep)]
        assert both(assessment(metric_claim(0.12), ev)) == (INS, SUP)
    ev = [records(out, dependence="clustered", design_effect=1.5)]
    assert both(assessment(metric_claim(0.12), ev)) == (SUP, SUP)


def test_outcome_records_temporal_zero_events():
    ev = [records([False] * 720, dependence="temporal")]
    # R: CP with no visible dependence (~0.0042); S assumes only sqrt(n) units (~0.106)
    assert both(assessment(metric_claim(0.01), ev)) == (SUP, INS)
    assert both(assessment(metric_claim(0.12), ev)) == (SUP, SUP)
    ev_de = [records([False] * 720, dependence="temporal", design_effect=2.0)]
    assert both(assessment(metric_claim(0.01), ev_de)) == (SUP, SUP)


def test_mismatched_conditions_never_supported():
    ev = [report(5000, 10, conditions=["2014-09"])]
    assert both(assessment(metric_claim(0.5, conditions=["2014-10"]), ev)) == (INS, INS)
    assert both(assessment(metric_claim(0.5, conditions=["2014-09", "2014-10"]), ev)) == (INS, INS)
    ev2 = [report(5000, 10, conditions=["2014-09", "2014-10"])]
    assert both(assessment(metric_claim(0.5, conditions=["2014-09"]), ev2)) == (INS, INS)


def test_disjoint_partition_and_overlap():
    ev = [report(1000, 20, conditions=["2014-09"], eid="a"),
          report(1000, 30, conditions=["2014-10"], eid="b")]
    claim = metric_claim(0.05, conditions=["2014-09", "2014-10"])
    assert both(assessment(claim, ev)) == (SUP, SUP)
    # one part too high -> not endorsed
    ev_hi = [report(1000, 20, conditions=["2014-09"], eid="a"),
             report(1000, 60, conditions=["2014-10"], eid="b")]
    assert both(assessment(claim, ev_hi)) == (INS, INS)
    # overlapping parts are not a partition
    ev_ov = [report(1000, 20, conditions=["2014-09"], eid="a"),
             report(1000, 20, conditions=["2014-09", "2014-10"], eid="b")]
    claim3 = metric_claim(0.05, conditions=["2014-09", "2014-10", "2014-11"])
    assert both(assessment(claim3, ev_ov)) == (INS, INS)


def test_s_prefers_raw_outcomes_over_naive_report():
    out = iid_outcomes(1000, 0.04, seed=5)
    ev = [records(out, dependence="independent", eid="raw"),
          report(1000, sum(out), dependence="temporal", interval=None, eid="rep")]
    assert m.review_s(assessment(metric_claim(0.065), ev)) == SUP
    assert m.review_s(assessment(metric_claim(0.065), ev[1:])) == INS


def test_population_mismatch_and_metric():
    ev = [report(1000, 50, population="other")]
    assert both(assessment(metric_claim(0.2), ev)) == (INS, INS)
    c = metric_claim(0.2)
    c["metric"] = "accuracy"
    assert both(assessment(c, [report(1000, 50)])) == (NA, NA)


def test_missing_and_malformed_inputs():
    ev = [report(1000, 50)]
    a = assessment(metric_claim(0.2), ev)
    a["claims"][0]["evidence"] = []
    assert both(a) == (INS, INS)
    a2 = assessment(metric_claim(0.2), ev)
    a2["claims"][0]["evidence"] = ["missing-id"]
    assert both(a2) == (INS, INS)
    assert both(assessment(metric_claim(0.2), [report(10, 50)])) == (INS, INS)
    assert both(assessment(metric_claim(None), ev)) == (INS, INS)
    rec = records([True, False] * 20)
    rec["outcomes"][3] = "x"
    bad = assessment(metric_claim(0.2), [rec])
    assert both(bad) == (INS, INS)
    c = metric_claim(0.2)
    c["confidence"] = 1.5
    assert both(assessment(c, ev)) == (INS, INS)


@pytest.mark.parametrize("junk", [None, {}, [], "x", {"claims": None},
                                  {"claims": [{}], "evidence": []},
                                  {"claims": [{"type": "metric_bound", "evidence": ["e"]}],
                                   "evidence": [{"id": "e", "kind": "metric_report"}]},
                                  {"claims": [{"type": "weird", "evidence": ["e"]}],
                                   "evidence": [{"id": "e"}]}])
def test_never_crashes(junk):
    assert m.review_r(junk) in m.VERDICTS
    assert m.review_s(junk) in m.VERDICTS


def test_fuzz_mutations_never_crash():
    rng = np.random.default_rng(0)
    base = assessment(metric_claim(0.07), [report(1000, 50, dependence="temporal",
                                                  interval=iv("block_bootstrap", .03, .07))])
    vals = [None, -1, 0, 1, 2.5, float("nan"), "s", [], {}, True, 10 ** 9]
    for _ in range(200):
        a = copy.deepcopy(base)
        target = a["evidence"][0] if rng.random() < 0.6 else a["claims"][0]
        key = list(target)[rng.integers(len(target))]
        target[key] = vals[rng.integers(len(vals))]
        assert m.review_r(a) in m.VERDICTS
        assert m.review_s(a) in m.VERDICTS


# ---------------------------------------------------------------- coverage_guarantee

def calibration(n=640, conditions=("2014-09",), counts=None, method=None,
                dependence="temporal", eid="cal", population=POP):
    counts = counts if counts is not None else {conditions[0]: n} if len(conditions) == 1 else {}
    return {"kind": "calibration", "id": eid, "population": population,
            "conditions": list(conditions), "condition_counts": counts, "n": n, "unit": "hour",
            "dependence": dependence,
            "method": method or "split conformal, absolute log1p residual, level 0.90"}


def shift(value, threshold=0.25, eid="shift"):
    return {"kind": "shift_diagnostic", "id": eid, "reference": "2014-09", "target": "2014-10",
            "statistic": "population stability index of predictions (10 reference-decile bins)",
            "value": value, "threshold": threshold}


def coverage_claim(conditions=("2014-10",), level=0.9, population=POP):
    return {"type": "coverage_guarantee", "id": "claim", "statement": "s",
            "population": population, "conditions": list(conditions), "level": level}


def test_parse_method_level():
    assert m.parse_method_level("split conformal, level 0.90") == pytest.approx(0.9)
    assert m.parse_method_level("split conformal at 90% level") == pytest.approx(0.9)
    assert m.parse_method_level("split conformal, level=95") == pytest.approx(0.95)
    assert m.parse_method_level("split conformal") is None


def test_coverage_same_strata():
    ev = [calibration(n=2000)]
    a = assessment(coverage_claim(conditions=["2014-09"]), ev)
    assert both(a) == (SUP, SUP)
    # smaller calibration: S's finite-sample risk too high, R's size rule passes
    ev_small = [calibration(n=250)]
    assert both(assessment(coverage_claim(conditions=["2014-09"]), ev_small)) == (SUP, INS)
    # n tiny
    ev_tiny = [calibration(n=5)]
    assert both(assessment(coverage_claim(conditions=["2014-09"]), ev_tiny)) == (INS, INS)


def test_coverage_level_above_calibration_level_contradicted():
    a = assessment(coverage_claim(conditions=["2014-09"], level=0.95), [calibration(n=2000)])
    assert both(a) == (CON, CON)
    a2 = assessment(coverage_claim(level=0.95), [calibration(n=2000), shift(0.01)])
    assert both(a2) == (INS, INS)


def test_coverage_cross_strata_shift_rules():
    # no diagnostic: neither endorses a guarantee for another month
    assert both(assessment(coverage_claim(), [calibration()])) == (INS, INS)
    # material shift
    assert both(assessment(coverage_claim(), [calibration(), shift(0.31)])) == (INS, INS)
    # below threshold: R's checklist passes; S charges a shift allowance and declines
    assert both(assessment(coverage_claim(), [calibration(), shift(0.10)])) == (SUP, INS)
    # large calibration and negligible shift: S also endorses
    assert both(assessment(coverage_claim(), [calibration(n=20000), shift(0.004)])) == (SUP, SUP)
    # claimed level well under calibration level
    a = assessment(coverage_claim(level=0.8), [calibration(), shift(0.05)])
    assert both(a) == (SUP, SUP)
    # shift above threshold vetoes even same-strata claims
    a = assessment(coverage_claim(conditions=["2014-09"]), [calibration(n=2000), shift(0.4)])
    assert both(a) == (INS, INS)


def test_coverage_subset_of_calibration_strata():
    cal = calibration(n=2000, conditions=["2014-08", "2014-09"],
                      counts={"2014-08": 1000, "2014-09": 1000})
    a = assessment(coverage_claim(conditions=["2014-09"], level=0.85), [cal])
    # R needs identical strata or a diagnostic; S's worst-case share bound: 1-(0.1+)/0.5 < 0.83
    assert both(a) == (INS, INS)
    a = assessment(coverage_claim(conditions=["2014-09"], level=0.75), [cal])
    assert m.review_s(a) == SUP


def test_coverage_malformed_and_not_applicable():
    a = assessment(coverage_claim(), [calibration(method="split conformal"), shift(0.01)])
    assert both(a) == (INS, INS)
    a = assessment(coverage_claim(), [calibration(method="quantile regression, level 0.9"), shift(0.01)])
    assert both(a) == (NA, NA)
    a = assessment(coverage_claim(level=None), [calibration(), shift(0.01)])
    assert both(a) == (INS, INS)
    a = assessment(coverage_claim(population="other"), [calibration(), shift(0.01)])
    assert both(a) == (INS, INS)
    a = assessment(coverage_claim(), [calibration(dependence="unknown", n=20000), shift(0.004)])
    assert both(a) == (INS, SUP)


# ---------------------------------------------------------------- operational_risk

REC_POP = "model-x/recent"


def op_claim(bound, condition="2014-11", min_support=100):
    return {"type": "operational_risk", "id": "claim", "statement": "s",
            "population": POP, "recent_population": REC_POP, "condition": condition,
            "bound": bound, "confidence": 0.95, "min_condition_support": min_support}


def op_cal(nov=300):
    return {"kind": "calibration", "id": "cal", "population": POP,
            "conditions": ["2014-10", "2014-11"], "condition_counts": {"2014-10": 400, "2014-11": nov},
            "n": 400 + nov, "unit": "hour", "dependence": "temporal",
            "method": "split conformal, absolute log1p residual, level 0.90"}


def recent(k=10, n=168, dependence="temporal"):
    out = [False] * n
    for i in range(k):
        out[(i * n) // k] = True  # evenly spread errors
    return records(out, dependence=dependence, conditions=["2014-11"], eid="recent",
                   population=REC_POP)


def completeness(required=152, available=168):
    return {"kind": "completeness", "id": "comp", "required": required, "available": available}


def support(value=0.08, support_max=0.10):
    return {"kind": "support_check", "id": "sup",
            "statistic": "mean predicted error probability, previous week",
            "value": value, "support_max": support_max}


def op_ev(**kw):
    ev = [kw.get("cal", op_cal()), kw.get("rec", recent()), kw.get("comp", completeness()),
          kw.get("sup", support())]
    return [e for e in ev if e is not None] + kw.get("extra", [])


def test_operational_supported_and_tight_bound():
    assert both(assessment(op_claim(0.2), op_ev())) == (SUP, SUP)
    r_ub = m.cp_upper(10, 168, 0.05)  # ~0.099 (no visible serial dependence)
    s_ub, _ = m.predictive_bounds(10 / 168, 168, 0.05)  # predictive is wider
    assert s_ub > r_ub
    mid = (r_ub + s_ub) / 2
    assert both(assessment(op_claim(mid), op_ev())) == (SUP, INS)


def test_operational_contradicted():
    a = assessment(op_claim(0.2), op_ev(rec=recent(k=80), sup=support(0.05)))
    assert both(a) == (CON, CON)


def test_operational_gates():
    assert both(assessment(op_claim(0.2), op_ev(sup=support(0.12)))) == (INS, INS)
    assert both(assessment(op_claim(0.2), op_ev(comp=completeness(available=120)))) == (INS, INS)
    assert both(assessment(op_claim(0.2), op_ev(cal=op_cal(nov=40)))) == (INS, INS)
    assert both(assessment(op_claim(0.2, condition="2014-12"), op_ev())) == (INS, INS)
    assert both(assessment(op_claim(0.2), op_ev(extra=[shift(0.4)]))) == (INS, INS)
    assert both(assessment(op_claim(0.2), op_ev(extra=[shift(0.1)]))) == (SUP, SUP)
    # recent sample shorter than the author's requirement
    assert both(assessment(op_claim(0.2), op_ev(rec=recent(n=100)))) == (INS, INS)


def test_operational_missing_items():
    assert both(assessment(op_claim(0.2), op_ev(cal=None))) == (INS, INS)
    assert both(assessment(op_claim(0.2), op_ev(rec=None))) == (INS, INS)
    # monitoring items are mandatory for R's checklist, optional for S
    assert both(assessment(op_claim(0.2), op_ev(comp=None))) == (INS, SUP)
    assert both(assessment(op_claim(0.2), op_ev(sup=None))) == (INS, SUP)
    c = op_claim(0.2)
    del c["min_condition_support"]
    assert both(assessment(c, op_ev())) == (INS, INS)


def test_operational_predicted_error_above_bound():
    a = assessment(op_claim(0.2), op_ev(sup=support(value=0.22, support_max=0.30)))
    assert both(a) == (SUP, INS)


def test_operational_recent_report_forms():
    rep_naive = report(168, 10, dependence="temporal", interval=iv("wilson", 0.03, 0.1),
                       conditions=["2014-11"], eid="recent", population=REC_POP)
    assert both(assessment(op_claim(0.35), op_ev(rec=rep_naive))) == (INS, SUP)
    assert both(assessment(op_claim(0.25), op_ev(rec=rep_naive))) == (INS, INS)
    rep_boot = report(168, 10, dependence="temporal", interval=iv("block_bootstrap", 0.02, 0.11),
                      conditions=["2014-11"], eid="recent", population=REC_POP)
    assert both(assessment(op_claim(0.2), op_ev(rec=rep_boot))) == (SUP, SUP)


# ---------------------------------------------------------------- determinism and CLI

def test_review_s_deterministic():
    out = runs_outcomes(n=1500, run=30, nruns=6, seed=2)
    a = assessment(metric_claim(0.2), [records(out, dependence="temporal")])
    assert len({m.review_s(a, seed_key="c1") for _ in range(3)}) == 1


def test_cli_round_trip(tmp_path):
    cases = {"schema_version": "triad-evidence-outcomes-cases-1", "cases": [
        {"case_id": "c-metric", "assessment": assessment(metric_claim(0.07), [report(1000, 50)])},
        {"case_id": "c-cov", "assessment": assessment(coverage_claim(), [calibration(), shift(0.31)])},
        {"case_id": "c-op", "assessment": assessment(op_claim(0.2), op_ev(comp=None))},
        {"case_id": "c-bad", "assessment": None},
    ]}
    inp, outp = tmp_path / "cases.json", tmp_path / "out.json"
    inp.write_text(json.dumps(cases))
    cmd = [sys.executable, os.path.join(HERE, "m8_comparators.py"),
           "--input", str(inp), "--output", str(outp)]
    subprocess.run(cmd, check=True, cwd=str(tmp_path))
    first = outp.read_text()
    res = json.loads(first)
    assert res["schema_version"] == "triad-m8-comparators-1"
    assert [r["case_id"] for r in res["results"]] == ["c-metric", "c-cov", "c-op", "c-bad"]
    assert [(r["r"], r["s"]) for r in res["results"]] == [
        (SUP, SUP), (INS, INS), (INS, SUP), (INS, INS)]
    subprocess.run(cmd, check=True, cwd=str(tmp_path))
    assert outp.read_text() == first
    assert res == m.run_cases(cases)
