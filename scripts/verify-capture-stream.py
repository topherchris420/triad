#!/usr/bin/env python3
"""Independent standard-library reference for `triad stream-capture`.

  python scripts/verify-capture-stream.py CAPTURE.json STREAM.json

Recomputes every causal per-frame feature directly from the capture with plain
loops (no flexplan, no shared code) and compares it with the Rust report:
rolling reference mean and sample sd, initial-baseline drift, drift score,
target z-scores, staleness and counts, plus null/non-null agreement. It also
re-checks causality by truncating the capture. Exit 1 on any mismatch.

Sums are accumulated left to right in explicit loops: since Python 3.12 the
built-in sum() uses compensated summation, which would not mirror the Rust
arithmetic.
"""
import json
import math
import sys
from pathlib import Path

CH = 10
RESOLUTION = 16.0 * sys.float_info.epsilon
TOL = 1e-12


def seq_sum(values):
    total = 0.0
    for v in values:
        total += v
    return total


def mean(values):
    return seq_sum(values) / len(values) if values else None


def sample_sd(values):
    if len(values) < 2:
        return None
    m = mean(values)
    var = seq_sum((v - m) * (v - m) for v in values) / (len(values) - 1)
    return math.sqrt(max(var, 0.0))


def normalized(x, sd, scale):
    """Mirror of capture_stream::normalized; None means undefined."""
    if x is None or sd is None or scale is None:
        return None
    floor = RESOLUTION * abs(scale)
    x = 0.0 if abs(x) <= floor else x
    sd = 0.0 if sd <= floor else sd
    if sd > 0.0:
        return x / sd
    return 0.0 if x == 0.0 else None


def reference_features(capture, window, initial_k):
    """Expected per-frame features, computed naively from scratch at every frame."""
    out = []
    refs = []  # all empty-reference frames so far
    last_ref_ms = None
    for index, frame in enumerate(capture["frames"]):
        role = frame["role"]
        if role == "empty_reference":
            refs.append(frame)
            last_ref_ms = frame["timestamp_ms"]
        recent = refs[-window:]
        chans = [[f["channels"][j] for f in recent] for j in range(CH)]
        ref_mean = [mean(c) for c in chans] if recent else None
        ref_sd = [sample_sd(c) for c in chans] if len(recent) >= 2 else None
        if len(refs) >= initial_k:
            init = [seq_sum(f["channels"][j] for f in refs[:initial_k]) / initial_k for j in range(CH)]
            drift = [ref_mean[j] - init[j] for j in range(CH)]
        else:
            drift = None
        score = None
        if drift is not None and ref_sd is not None:
            vals = [normalized(drift[j], ref_sd[j], ref_mean[j]) for j in range(CH)]
            if all(v is not None for v in vals):
                score = 0.0
                for v in vals:
                    score = max(score, abs(v))
        z = max_z = None
        if role == "target_present":
            # Statistics from before this frame (a target never updates the reference).
            z = []
            for j in range(CH):
                if ref_mean is None or ref_sd is None:
                    z.append(None)
                else:
                    z.append(normalized(frame["channels"][j] - ref_mean[j], ref_sd[j], ref_mean[j]))
            finite = [abs(v) for v in z if v is not None]
            max_z = max(finite) if finite else None
        since = None if last_ref_ms is None else (frame["timestamp_ms"] - last_ref_ms) * 1.0
        out.append({
            "index": index,
            "timestamp_ms": frame["timestamp_ms"],
            "role": role,
            "reference_frames_seen": len(refs),
            "reference_frames_in_window": len(recent),
            "ms_since_reference": since,
            "reference_mean": ref_mean,
            "reference_stddev": ref_sd,
            "drift_from_initial": drift,
            "drift_score": score,
            "target_z": z,
            "max_abs_target_z": max_z,
        })
    return out


def close(a, b):
    return abs(a - b) <= TOL * max(1.0, abs(a), abs(b))


def compare(path, expected, actual, errors):
    if isinstance(expected, list) or isinstance(actual, list):
        if not isinstance(expected, list) or not isinstance(actual, list) or len(expected) != len(actual):
            errors.append(f"{path}: expected {expected!r}, got {actual!r}")
            return
        for i, (e, a) in enumerate(zip(expected, actual)):
            compare(f"{path}[{i}]", e, a, errors)
    elif expected is None or actual is None:
        if expected is not actual:
            errors.append(f"{path}: expected {expected!r}, got {actual!r}")
    elif isinstance(expected, float) or isinstance(actual, float):
        if not close(float(expected), float(actual)):
            errors.append(f"{path}: expected {expected!r}, got {actual!r}")
    elif expected != actual:
        errors.append(f"{path}: expected {expected!r}, got {actual!r}")


def main():
    if len(sys.argv) != 3:
        raise SystemExit("usage: verify-capture-stream.py CAPTURE.json STREAM.json")
    capture = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    report = json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))
    errors = []
    if report["schema_version"] != "triad-capture-stream-1":
        errors.append("schema_version")
    if report["capture_id"] != capture["capture_id"]:
        errors.append("capture_id")
    if "unvalidated candidate refusal score" not in report["warning"]:
        errors.append("warning text must state that the drift score is unvalidated")
    window = report["config"]["window"]
    k = report["config"]["initial_reference_frames"]
    expected = reference_features(capture, window, k)
    if len(expected) != len(report["frames"]):
        errors.append(f"frame count {len(report['frames'])} != {len(expected)}")
    for e, a in zip(expected, report["frames"]):
        for key, value in e.items():
            compare(f"frames[{e['index']}].{key}", value, a.get(key), errors)
        extra = set(a) - set(e)
        if extra:
            errors.append(f"frames[{e['index']}]: unexpected fields {sorted(extra)}")
    # Causality: the naive reference on any prefix equals its own full run on that prefix.
    for cut in range(0, len(capture["frames"]), max(1, len(capture["frames"]) // 7)):
        prefix = dict(capture, frames=capture["frames"][: cut + 1])
        if reference_features(prefix, window, k) != expected[: cut + 1]:
            errors.append(f"reference implementation is not causal at {cut}")
    if errors:
        for line in errors[:40]:
            print("FAIL:", line)
        print(f"FAIL: {len(errors)} mismatches")
        sys.exit(1)
    print(f"PASS: {len(expected)} frames of causal capture-stream features match the independent reference")


if __name__ == "__main__":
    main()
