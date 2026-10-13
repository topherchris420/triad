#!/usr/bin/env python3
"""Check that the quickstart's Python reference agrees with `triad metrology evaluate`.

docs/metrology/selective-risk-quickstart.md publishes a standard-library Python
function for selective risk at target coverage. This script extracts the block that
defines it, runs it and TRIAD on the shipped example and on randomized inputs, and
requires identical threshold, accepted count, accepted-error count, realised
coverage and selective risk. The randomized inputs include score ties at the
threshold, classification and Euclidean losses, c = 0 and c = 1, and calibration
rows outside the calibration scenario, which TRIAD must ignore.
It also checks that empty splits and mismatched vectors are rejected, including
the classifier usage example's label-length guard.

  python scripts/check-quickstart-reference.py [--triad PATH] [--cases N] [--seed S]

Standard library only. Build TRIAD first: cargo build --release --locked
"""
import argparse
import copy
import functools
import json
import math
import random
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
QUICKSTART = ROOT / "docs/metrology/selective-risk-quickstart.md"
EXAMPLE = ROOT / "examples/predictions.json"


def load_reference():
    blocks = re.findall(r"```python\n(.*?)```", QUICKSTART.read_text(encoding="utf-8"), re.S)
    source = [b for b in blocks if "def selective_risk_at_coverage" in b]
    if len(source) != 1:
        sys.exit("FAIL: expected exactly one Python block defining selective_risk_at_coverage")
    namespace = {}
    exec(compile(source[0], str(QUICKSTART), "exec"), namespace)
    return namespace["selective_risk_at_coverage"]


def check_reference_contract(fn):
    """Exercise malformed standalone inputs that record-based parity cases cannot produce."""
    invalid = []
    for c in (0.0, 0.5, 1.0):
        invalid.extend([
            (f"empty calibration, c={c}", ([], [0.1], [1], c), "cal_scores must not be empty"),
            (f"empty test, c={c}", ([0.1], [], [], c), "test_scores must not be empty"),
            (f"short error vector, c={c}", ([0.1], [0.1, 0.1], [1], c),
             "test_scores and test_errors must have equal lengths"),
            (f"long error vector, c={c}", ([0.1], [0.1], [1, 1], c),
             "test_scores and test_errors must have equal lengths"),
        ])
    for name, args, message in invalid:
        try:
            fn(*args)
        except ValueError as error:
            if str(error) != message:
                sys.exit(f"FAIL: {name}: expected {message!r}, got {str(error)!r}")
        else:
            sys.exit(f"FAIL: reference accepted {name}")

    empty_acceptance = {"threshold": None, "coverage": 0.0, "selective_risk": None,
                        "accepted": 0, "accepted_errors": 0}
    if fn([0.1], [0.1], [1], 0.0) != empty_acceptance:
        sys.exit("FAIL: c = 0 must reject all rows with undefined selective risk")
    if fn([0.1], [0.2], [1], 1.0) != dict(empty_acceptance, threshold=0.1):
        sys.exit("FAIL: a fitted threshold accepting no test rows must have undefined risk")

    blocks = re.findall(r"```python\n(.*?)```", QUICKSTART.read_text(encoding="utf-8"), re.S)
    examples = [b for b in blocks if "result = selective_risk_at_coverage(" in b]
    if len(examples) != 1:
        sys.exit("FAIL: expected exactly one classifier usage example")
    example = compile(examples[0], str(QUICKSTART), "exec")
    for pred, truth, scores in (([0, 1], [1], [0.1]), ([0], [1, 1], [0.1]),
                                ([0], [1], [0.1, 0.2])):
        namespace = {"selective_risk_at_coverage": fn, "cal_scores": [0.1],
                     "test_scores": scores, "test_pred": pred, "test_true": truth}
        try:
            exec(example, namespace)
        except ValueError as error:
            if str(error) != "test_pred, test_true and test_scores must have equal lengths":
                sys.exit(f"FAIL: classifier example rejected mismatched labels incorrectly: {error}")
        else:
            sys.exit("FAIL: classifier example silently truncated mismatched labels")
    namespace = {"selective_risk_at_coverage": fn, "cal_scores": [0.1],
                 "test_scores": [0.1, 0.2], "test_pred": [0, 1], "test_true": [1, 1]}
    exec(example, namespace)
    expected = {"threshold": 0.1, "coverage": 0.5, "selective_risk": 1.0,
                "accepted": 1, "accepted_errors": 1}
    if namespace["result"] != expected:
        sys.exit("FAIL: classifier usage example returned incorrect metrics")
    print(f"PASS: reference rejects {len(invalid)} malformed inputs and checks classifier label alignment")


def loss_event(record, inp):
    """Mirror of `metrology::event`: the loss exceeds the declared tolerance."""
    if inp["loss"] == "classification":
        loss = 0.0 if record["prediction"] == record["truth"] else 1.0
    else:
        diffs = [a - b for a, b in zip(record["prediction"], record["truth"])]
        loss = functools.reduce(math.hypot, diffs, 0.0)
    return int(loss > inp["tolerance"])


def reference_result(fn, inp):
    calibration = [r["uncertainty"] for r in inp["records"]
                   if r["split"] == "calibration" and r["scenario"] == inp["calibration_scenario"]]
    test = sorted((r for r in inp["records"] if r["split"] == "test"), key=lambda r: r["id"])
    return fn(calibration, [r["uncertainty"] for r in test],
              [loss_event(r, inp) for r in test], inp["target_coverage"])


def triad_result(binary, inp, workdir, name):
    source, output = workdir / f"{name}-input.json", workdir / f"{name}-report.json"
    source.write_text(json.dumps(inp), encoding="utf-8")
    run = subprocess.run([str(binary), "metrology", "evaluate", "--input", str(source),
                          "--output", str(output)], capture_output=True, text=True)
    if run.returncode != 0:
        sys.exit(f"FAIL: TRIAD rejected case {name}:\n{run.stderr}")
    report = json.loads(output.read_text(encoding="utf-8"))["report"]
    operational = report["overall"]["operational"]
    return {"threshold": report["threshold"], "coverage": operational["coverage"],
            "selective_risk": operational["selective_error_risk"],
            "accepted": operational["accepted"], "accepted_errors": operational["accepted_errors"]}


def random_case(rng, template, index):
    loss = rng.choice(["classification", "euclidean"])
    dim = 1 if loss == "classification" else rng.randint(1, 3)
    if rng.random() < 0.6:  # coarse grid: many ties, including at the threshold
        levels = rng.randint(1, 6)
        score = lambda: rng.randint(0, levels) / levels - (0.5 if rng.random() < 0.2 else 0.0)
    else:
        score = lambda: rng.uniform(-1.0, 2.0)

    def record(rid, split, scenario):
        if loss == "classification":
            pred, truth = [float(rng.randint(0, 2))], [float(rng.randint(0, 2))]
        else:
            pred = [rng.uniform(-2, 2) for _ in range(dim)]
            truth = [rng.uniform(-2, 2) for _ in range(dim)]
        return {"id": rid, "prediction": pred, "truth": truth, "uncertainty": score(),
                "error_probability": None, "sequence": rid, "spatial_cluster": None,
                "split": split, "scenario": scenario}

    n_cal = rng.randint(1, 30)
    records = [record(f"cal-{i}", "calibration", "iid") for i in range(n_cal)]
    records += [record(f"cal-other-{i}", "calibration", "other") for i in range(rng.randint(0, 5))]
    records += [record(f"test-{i}", "test", rng.choice(["iid", "shift"]))
                for i in range(rng.randint(1, 40))]
    c = rng.choice([0.0, 1.0, rng.random(), round(rng.random(), 1), rng.randint(0, n_cal) / n_cal])
    inp = copy.deepcopy(template)
    inp.update({"model_id": "reference-check", "experiment_id": f"random-{index}", "loss": loss,
                "tolerance": 0.0 if loss == "classification" else rng.uniform(0.0, 2.5),
                "target_coverage": c, "calibration_scenario": "iid",
                "bootstrap_replicates": 100, "bootstrap_seed": index, "records": records})
    return inp


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--triad", type=Path, help="TRIAD binary (default: target/release/triad[.exe])")
    ap.add_argument("--cases", type=int, default=300, help="randomized cases (default 300)")
    ap.add_argument("--seed", type=int, default=20261013)
    args = ap.parse_args()
    binary = args.triad or next((p for p in (ROOT / "target/release/triad", ROOT / "target/release/triad.exe")
                                 if p.exists()), None)
    if binary is None or not Path(binary).exists():
        sys.exit("FAIL: TRIAD binary not found; run cargo build --release --locked or pass --triad")

    fn = load_reference()
    check_reference_contract(fn)
    template = json.loads(EXAMPLE.read_text(encoding="utf-8"))
    rng = random.Random(args.seed)
    cases = [("example", template)] + [(f"random-{i}", random_case(rng, template, i))
                                       for i in range(args.cases)]
    stats = {"tie at threshold": 0, "c = 0": 0, "c = 1": 0, "euclidean": 0, "zero accepted": 0}
    with tempfile.TemporaryDirectory() as tmp:
        for name, inp in cases:
            ours, theirs = reference_result(fn, inp), triad_result(binary, inp, Path(tmp), name)
            if ours != theirs:
                sys.exit(f"FAIL: case {name} differs\n  python: {ours}\n  triad:  {theirs}")
            test_scores = [r["uncertainty"] for r in inp["records"] if r["split"] == "test"]
            stats["tie at threshold"] += ours["threshold"] is not None and ours["threshold"] in test_scores
            stats["c = 0"] += inp["target_coverage"] == 0.0
            stats["c = 1"] += inp["target_coverage"] == 1.0
            stats["euclidean"] += inp["loss"] == "euclidean"
            stats["zero accepted"] += ours["accepted"] == 0
    detail = ", ".join(f"{v} {k}" for k, v in stats.items())
    print(f"PASS: quickstart reference matches TRIAD on {len(cases)} cases ({detail})")


if __name__ == "__main__":
    main()
