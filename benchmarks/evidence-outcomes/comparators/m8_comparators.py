"""Two independent reviewers (R and S) of reliability claims against evidence.

Each reviewer reads one assessment (exactly one claim plus evidence items) and
returns one of: ``supported_within_scope``, ``insufficient_evidence``,
``contradicted`` or ``not_applicable``.  Only the evidence items whose ids are
listed in the claim's ``evidence`` field are considered.  Any missing, malformed
or out-of-range required input gives ``insufficient_evidence``; neither reviewer
ever raises on a well-formed (or badly formed) assessment.  ``alpha`` below is
``1 - confidence`` (confidence defaults to 0.95 when absent; it must lie in
(0.5, 1)).  "Bound" means a one-sided bound at level ``1 - alpha``.

Shared scope rules (both reviewers)
-----------------------------------
* Evidence counts for a claim only if its ``population`` equals the claim's
  population and its ``metric`` is ``material_error_rate``.  A claim whose metric
  is something else is ``not_applicable``; so is a coverage claim whose linked
  calibration explicitly describes a non-conformal method.
* metric_bound: the claim is about the actual rate over *all* units of the named
  strata.  Evidence about other strata never supports it: a rate over strata A
  says nothing (at 95%) about strata B, and a rate pooled over A+B does not bound
  the rate over A alone because the stratum weights are unknown.  An item is in
  scope only if its conditions are a subset of the claim's.  Either some items
  cover exactly the claim's strata, or in-scope items with pairwise-disjoint
  condition sets must together cover all of them (a stratified claim: the
  claimed rate is a weighted average of the parts, so it is at most the largest
  part's upper bound and at least the smallest part's lower bound).
* Endorse ``at_most`` when the upper bound is <= the claimed bound; return
  ``contradicted`` when the lower bound exceeds it (mirror for ``at_least``).

Reviewer R: REFORMS-style checklist
-----------------------------------
R ticks explicit checklist items and endorses only if every item passes.
1. Representativeness / scope: the shared scope rules above; for coverage
   claims the target strata must equal the calibration strata, otherwise the
   generalisation is only accepted when a linked label-free shift diagnostic is
   present and every one is at or below its threshold.
2. Sample size: every outcome item needs n >= 30 units (n tiny -> insufficient).
3. Uncertainty respects data collection (one item at a time):
   - a declared design effect D (>= 1) is always accepted: Clopper-Pearson (CP)
     bound with effective size n/D and events/D;
   - ``independent``: CP bound from the counts (reported intervals ignored);
   - ``temporal``: a reported ``block_bootstrap`` interval (two-sided level
     >= 0.90, its limits used as given), or, from raw outcomes, a CP bound with
     the AR(1) design effect (1+rho)/(1-rho) from the lag-1 autocorrelation
     (rho clipped to [0, 0.95]);
   - ``clustered``: a reported ``cluster_bootstrap`` interval (level >= 0.90);
     raw outcomes without D are rejected (cluster boundaries are unknown);
   - ``unknown`` dependence without D, and any iid interval (clopper_pearson,
     wilson, normal_iid) on dependent data, are rejected.
   When several usable items cover the same strata, all must pass (largest upper
   bound is used); a stratified claim uses each part's own 95% bound.
4. Coverage claims: calibration evidence present, method contains "conformal"
   and a parsable level, claimed level <= calibration level, calibration
   dependence stated (not ``unknown``), n >= 20/(1 - level), and no linked shift
   diagnostic above its threshold.  If target strata equal calibration strata
   and the claimed level minus 0.02 exceeds level_cal + 1/(n+1): contradicted.
5. Operational claims: all monitoring items must be linked and pass:
   calibration support for the coming month >= min_condition_support,
   completeness available >= required, support_check value <= support_max,
   every linked shift diagnostic <= threshold, recent n >= required; then the
   confidence bound on the previous week's rate (rules of item 3) must be <=
   the bound.  Recent lower bound above the bound: contradicted.

Reviewer S: applied statistician
--------------------------------
S computes the most appropriate bound each piece of evidence allows.
* Binomial bounds are exact (Clopper-Pearson, beta quantiles; non-integer
  effective counts use the continuous extension).
* Raw outcomes, ``independent``: exact CP (times a declared design effect).
  Raw outcomes, dependent (clustered/temporal/unknown; consecutive units share
  clusters or are serially correlated): the design effect is estimated by
  batch means, D = max over block sizes b (with >= 20 blocks) of
  b * var(block means) / (p(1-p)), and combined with a declared D (larger one
  used); bound = max(CP with n/D, moving-block-bootstrap percentile bound,
  iid CP).  When the data cannot reveal its dependence (no events, all events,
  or n < 40) and no D is declared, S assumes only sqrt(n) effectively
  independent units.
* Summary reports: ``independent`` -> exact CP from counts.  Dependent ->
  dependence-aware evidence only: declared D (CP with n/D) and/or a block or
  cluster bootstrap interval (its upper limit used as given when its one-sided
  level (1+L)/2 reaches 1-alpha, otherwise rescaled to 1-alpha by normal
  quantiles); the largest of these and the iid CP bound is used.  If the only
  uncertainty statement assumes independence, S falls back to an effective
  sample size of sqrt(n).
* Several items on the same strata: the most direct evidence tier is used
  (computed from raw outcomes > dependence-aware report > sqrt(n) fallback),
  most conservative within a tier.  Stratified claims use Bonferroni-adjusted
  bounds (alpha / number of parts).
* Coverage claims (split conformal): under exchangeability the coverage given
  the calibration set is Beta(k, n+1-k), k = ceil((n+1) level_cal) (effective n
  = n/D when a design effect is declared; k > n -> insufficient).  The claim is
  true when realised coverage >= level - 0.02.  Same strata as calibration:
  endorse if P(coverage < level - 0.02) <= 0.10 and no linked shift diagnostic
  exceeds its threshold; contradicted if P(coverage >= level - 0.02) <= alpha.
  Target strata a strict subset of calibration strata: marginal coverage need
  not hold per stratum; with share w of the calibration units, miscoverage in
  the subset is at most (miscoverage)/w, and S endorses only if that worst case
  still meets level - 0.02 at the 10% Beta quantile.  Other strata:
  exchangeability is not guaranteed, so S requires every linked shift
  diagnostic at or below threshold and charges a covariate-shift allowance
  delta = 0.1 * sqrt(PSI) (total-variation of 10 equal-mass bins <= sqrt(PSI)/2,
  times an assumed 0.2 spread of conditional coverage across bins); it endorses
  if P(coverage < level - 0.02 + delta) <= 0.10.  No diagnostic -> insufficient.
* Operational claims: the coming week is a new realisation, so S uses a
  *predictive* bound: the previous week's (p, effective n) gives a Jeffreys
  posterior Beta(k_eff+1/2, n_eff-k_eff+1/2) and the next week's count over
  n_eff effective units is beta-binomial; its 1-alpha quantile / n_eff must be
  <= the bound (alpha quantile above the bound: contradicted).  Required:
  calibration with support for the coming month >= min_condition_support and
  matching recent outcomes.  Optional but binding if linked: completeness
  (available >= required, recent n >= required), support_check (value <=
  support_max; and when it is a predicted error probability, value <= bound),
  shift diagnostics (<= threshold).

Determinism: resampling uses numpy.random.default_rng seeded from SHA-256 of
the case id (CLI) or assessment id plus the evidence id.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import sys
from typing import Any

import numpy as np
from scipy import stats

SUPPORTED = "supported_within_scope"
INSUFFICIENT = "insufficient_evidence"
CONTRADICTED = "contradicted"
NOT_APPLICABLE = "not_applicable"
VERDICTS = (SUPPORTED, INSUFFICIENT, CONTRADICTED, NOT_APPLICABLE)

METRIC = "material_error_rate"
OUTCOME_KINDS = ("outcome_records", "metric_report")
IID_METHODS = ("clopper_pearson", "wilson", "normal_iid")
BOOT_METHODS = ("block_bootstrap", "cluster_bootstrap")
DEPENDENT = ("clustered", "temporal", "unknown")
COVERAGE_SLACK = 0.02

R_MIN_N = 30
S_COVERAGE_RISK = 0.10
S_SHIFT_SPREAD = 0.1
BOOT_REPS = 2000


# ---------------------------------------------------------------------------
# small parsing helpers
# ---------------------------------------------------------------------------

def _num(x: Any) -> float | None:
    if isinstance(x, bool) or not isinstance(x, (int, float)):
        return None
    x = float(x)
    return x if math.isfinite(x) else None


def _count(x: Any) -> int | None:
    v = _num(x)
    if v is None or v < 0 or abs(v - round(v)) > 1e-9:
        return None
    return int(round(v))


def _str(x: Any) -> str | None:
    return x.strip() if isinstance(x, str) and x.strip() else None


def _conds(x: Any) -> frozenset | None:
    if not isinstance(x, list) or not x:
        return None
    out = set()
    for c in x:
        s = _str(c)
        if s is None:
            return None
        out.add(s)
    return frozenset(out)


def _outcomes(x: Any) -> np.ndarray | None:
    if not isinstance(x, list) or not x:
        return None
    vals = []
    for v in x:
        if isinstance(v, bool):
            vals.append(1.0 if v else 0.0)
        elif isinstance(v, int) and v in (0, 1):
            vals.append(float(v))
        else:
            return None
    return np.asarray(vals, dtype=float)


def _alpha(claim: dict) -> float | None:
    c = claim.get("confidence", 0.95)
    if c is None:
        c = 0.95
    c = _num(c)
    if c is None or not (0.5 < c < 1.0):
        return None
    return 1.0 - c


def _seed(*parts: Any) -> int:
    key = "|".join(str(p) for p in parts)
    return int.from_bytes(hashlib.sha256(key.encode("utf-8")).digest()[:8], "little")


def _unpack(assessment: Any) -> tuple[dict, list[dict]] | None:
    """Return (claim, linked evidence items) or None when malformed."""
    if not isinstance(assessment, dict):
        return None
    claims = assessment.get("claims")
    if not isinstance(claims, list) or len(claims) != 1 or not isinstance(claims[0], dict):
        return None
    claim = claims[0]
    ids = claim.get("evidence")
    if not isinstance(ids, list):
        return None
    wanted = {i for i in ids if isinstance(i, str)}
    ev = assessment.get("evidence")
    if not isinstance(ev, list):
        return None
    items = [e for e in ev if isinstance(e, dict) and e.get("id") in wanted]
    return claim, items


def _design_effect(item: dict, strict: bool) -> tuple[bool, float | None]:
    """(ok, D).  strict: D < 1 is malformed; lenient: clipped to 1."""
    d = item.get("design_effect")
    if d is None:
        return True, None
    d = _num(d)
    if d is None or d <= 0:
        return False, None
    if d < 1:
        return (False, None) if strict else (True, 1.0)
    return True, d


def _report_counts(item: dict) -> tuple[int | None, float | None]:
    n = _count(item.get("n"))
    if n is None or n <= 0:
        return None, None
    ev = item.get("events")
    if ev is not None:
        k = _count(ev)
        if k is None or k > n:
            return n, None
        return n, float(k)
    v = _num(item.get("value"))
    if v is None or not (0 <= v <= 1):
        return n, None
    return n, v * n


def _interval(item: dict) -> dict | None:
    iv = item.get("interval")
    if not isinstance(iv, dict):
        return None
    lo, hi, lev = _num(iv.get("lower")), _num(iv.get("upper")), _num(iv.get("level"))
    method = _str(iv.get("method"))
    if None in (lo, hi, lev) or method is None:
        return None
    if not (0 <= lo <= hi <= 1) or not (0 < lev < 1):
        return None
    return {"lower": lo, "upper": hi, "level": lev, "method": method.lower()}


def _matches(item: dict, population: str, need_metric: bool = True) -> bool:
    if _str(item.get("population")) != population:
        return False
    if need_metric:
        m = item.get("metric")
        if m is not None and m != METRIC:
            return False
    return True


# ---------------------------------------------------------------------------
# binomial / dependence machinery
# ---------------------------------------------------------------------------

def cp_upper(k: float, n: float, alpha: float) -> float:
    """One-sided Clopper-Pearson upper bound (continuous in k, n)."""
    if n <= 0:
        return 1.0
    if k >= n:
        return 1.0
    return float(stats.beta.ppf(1 - alpha, k + 1, n - k))


def cp_lower(k: float, n: float, alpha: float) -> float:
    if n <= 0 or k <= 0:
        return 0.0
    return float(stats.beta.ppf(alpha, k, n - k + 1))


def ar1_design_effect(x: np.ndarray) -> float:
    if len(x) < 3:
        return 1.0
    m = x.mean()
    d = x - m
    den = float(np.sum(d * d))
    if den <= 0:
        return 1.0
    rho = float(np.sum(d[:-1] * d[1:]) / den)
    rho = min(max(rho, 0.0), 0.95)
    return (1 + rho) / (1 - rho)


def batch_means_design_effect(x: np.ndarray, min_blocks: int = 20) -> tuple[float, int] | None:
    """Largest batch-means variance-inflation estimate over block sizes."""
    n = len(x)
    p = x.mean()
    if n < 2 * min_blocks or p <= 0 or p >= 1:
        return None
    best, best_b = 1.0, 1
    b = 1
    sizes = []
    while n // b >= min_blocks:
        sizes.append(b)
        b = max(b + 1, int(round(b * 1.5)))
    for b in sizes:
        m = n // b
        blocks = x[: m * b].reshape(m, b).mean(axis=1)
        pp = blocks.mean()
        if pp <= 0 or pp >= 1:
            continue
        d = b * float(np.var(blocks, ddof=1)) / (pp * (1 - pp))
        if d > best:
            best, best_b = d, b
    return best, best_b


def mbb_bounds(x: np.ndarray, block: int, alpha: float, seed: int) -> tuple[float, float]:
    n = len(x)
    block = int(min(max(block, 1), n))
    nb = int(math.ceil(n / block))
    reps = BOOT_REPS if nb * BOOT_REPS <= 5_000_000 else max(200, 5_000_000 // nb)
    rng = np.random.default_rng(seed)
    cs = np.concatenate([[0.0], np.cumsum(x)])
    starts = rng.integers(0, n - block + 1, size=(reps, nb))
    sums = (cs[starts + block] - cs[starts]).sum(axis=1)
    means = sums / (nb * block)
    return float(np.quantile(means, 1 - alpha)), float(np.quantile(means, alpha))


def _interval_bounds(iv: dict, phat: float | None, alpha: float) -> tuple[float, float]:
    """One-sided (1-alpha) bounds from a two-sided interval; rescaled if too low."""
    one_sided = (1 + iv["level"]) / 2
    if one_sided >= 1 - alpha - 1e-12:
        return iv["upper"], iv["lower"]
    c = phat if phat is not None else (iv["upper"] + iv["lower"]) / 2
    c = min(max(c, iv["lower"]), iv["upper"])
    f = stats.norm.ppf(1 - alpha) / stats.norm.ppf(one_sided)
    return min(1.0, c + (iv["upper"] - c) * f), max(0.0, c - (c - iv["lower"]) * f)


# ---------------------------------------------------------------------------
# Reviewer R: per-item bounds
# ---------------------------------------------------------------------------

def _r_item(item: dict, alpha: float) -> tuple[float, float] | None:
    kind = item.get("kind")
    dep = item.get("dependence")
    ok, D = _design_effect(item, strict=True)
    if not ok or dep not in ("independent", "clustered", "temporal", "unknown"):
        return None
    if kind == "outcome_records":
        x = _outcomes(item.get("outcomes"))
        if x is None or len(x) < R_MIN_N:
            return None
        n, k = float(len(x)), float(x.sum())
        if D is None:
            if dep == "independent":
                D = 1.0
            elif dep == "temporal":
                D = ar1_design_effect(x)
            else:
                return None
        return cp_upper(k / D, n / D, alpha), cp_lower(k / D, n / D, alpha)
    if kind == "metric_report":
        n, k = _report_counts(item)
        iv = _interval(item)
        if n is not None and n < R_MIN_N:
            return None
        if D is not None and n is not None and k is not None:
            return cp_upper(k / D, n / D, alpha), cp_lower(k / D, n / D, alpha)
        if dep == "independent":
            if n is not None and k is not None:
                return cp_upper(k, n, alpha), cp_lower(k, n, alpha)
            if iv is not None and (1 + iv["level"]) / 2 >= 1 - alpha - 1e-12:
                return iv["upper"], iv["lower"]
            return None
        wanted = {"temporal": "block_bootstrap", "clustered": "cluster_bootstrap"}.get(dep)
        if iv is not None and iv["method"] == wanted and (1 + iv["level"]) / 2 >= 1 - alpha - 1e-12:
            if n is None:
                return None
            return iv["upper"], iv["lower"]
        return None
    return None


# ---------------------------------------------------------------------------
# Reviewer S: per-item bounds and effective sizes
# ---------------------------------------------------------------------------

def _s_effective(item: dict) -> tuple[float, float, int] | None:
    """(phat, n_eff, tier) without resampling; used for predictive bounds."""
    kind = item.get("kind")
    dep = item.get("dependence")
    ok, Dsup = _design_effect(item, strict=False)
    if not ok:
        return None
    if dep not in ("independent", "clustered", "temporal", "unknown"):
        dep = "unknown"
    if kind == "outcome_records":
        x = _outcomes(item.get("outcomes"))
        if x is None:
            return None
        n, k = float(len(x)), float(x.sum())
        if dep == "independent":
            D = Dsup or 1.0
            return k / n, n / D, 3
        bm = batch_means_design_effect(x)
        if bm is None:
            D = Dsup if Dsup is not None else max(1.0, math.sqrt(n))
            return k / n, n / D, 3 if Dsup is not None else 1
        D = max(bm[0], Dsup or 1.0)
        return k / n, n / D, 3
    if kind == "metric_report":
        n, k = _report_counts(item)
        if n is None or k is None:
            return None
        p = k / n
        if dep == "independent":
            return p, n / (Dsup or 1.0), 3
        cands = []
        if Dsup is not None:
            cands.append(Dsup)
        iv = _interval(item)
        if iv is not None and iv["method"] in BOOT_METHODS and 0 < p < 1:
            se = (iv["upper"] - iv["lower"]) / (2 * stats.norm.ppf((1 + iv["level"]) / 2))
            if se > 0:
                cands.append(max(1.0, n * se * se / (p * (1 - p))))
        if cands:
            return p, n / max(cands), 2
        return p, n / max(1.0, math.sqrt(n)), 1
    return None


def _s_item(item: dict, alpha: float, seed: int) -> tuple[float, float, int] | None:
    """(upper, lower, tier) at one-sided level 1-alpha."""
    kind = item.get("kind")
    dep = item.get("dependence")
    ok, Dsup = _design_effect(item, strict=False)
    if not ok:
        return None
    if dep not in ("independent", "clustered", "temporal", "unknown"):
        dep = "unknown"
    if kind == "outcome_records":
        x = _outcomes(item.get("outcomes"))
        if x is None:
            return None
        n, k = float(len(x)), float(x.sum())
        if dep == "independent":
            D = Dsup or 1.0
            return cp_upper(k / D, n / D, alpha), cp_lower(k / D, n / D, alpha), 3
        bm = batch_means_design_effect(x)
        if bm is None:
            if Dsup is not None:
                D, tier = Dsup, 3
            else:
                D, tier = max(1.0, math.sqrt(n)), 1
            return cp_upper(k / D, n / D, alpha), cp_lower(k / D, n / D, alpha), tier
        D = max(bm[0], Dsup or 1.0)
        block = max(int(math.ceil(n ** (1 / 3))), min(bm[1], int(n // 10)))
        bu, bl = mbb_bounds(x, block, alpha, seed)
        ub = max(cp_upper(k / D, n / D, alpha), bu, cp_upper(k, n, alpha))
        lb = min(cp_lower(k / D, n / D, alpha), bl, cp_lower(k, n, alpha))
        return ub, lb, 3
    if kind == "metric_report":
        n, k = _report_counts(item)
        iv = _interval(item)
        phat = (k / n) if (n and k is not None) else _num(item.get("value"))
        if dep == "independent":
            if n is not None and k is not None:
                D = Dsup or 1.0
                return cp_upper(k / D, n / D, alpha), cp_lower(k / D, n / D, alpha), 3
            if iv is not None:
                u, l = _interval_bounds(iv, phat, alpha)
                return u, l, 2
            return None
        ups, los = [], []
        if Dsup is not None and n is not None and k is not None:
            ups.append(cp_upper(k / Dsup, n / Dsup, alpha))
            los.append(cp_lower(k / Dsup, n / Dsup, alpha))
        if iv is not None and iv["method"] in BOOT_METHODS:
            u, l = _interval_bounds(iv, phat, alpha)
            ups.append(u)
            los.append(l)
        if ups:
            if n is not None and k is not None:
                ups.append(cp_upper(k, n, alpha))
                los.append(cp_lower(k, n, alpha))
            return max(ups), min(los), 2
        if n is None or k is None:
            return None
        D = max(1.0, math.sqrt(n))
        return cp_upper(k / D, n / D, alpha), cp_lower(k / D, n / D, alpha), 1
    return None


# ---------------------------------------------------------------------------
# metric_bound
# ---------------------------------------------------------------------------

def _metric_groups(claim: dict, items: list[dict]) -> tuple[list[list[dict]], str] | None:
    """Group in-scope items: either one exact group or a disjoint partition."""
    population = _str(claim.get("population"))
    C = _conds(claim.get("conditions"))
    if population is None or C is None:
        return None
    usable = []
    for it in items:
        if it.get("kind") not in OUTCOME_KINDS or not _matches(it, population):
            continue
        ec = _conds(it.get("conditions"))
        if ec is None or not ec <= C:
            continue
        usable.append((ec, it))
    exact = [it for ec, it in usable if ec == C]
    if exact:
        return [exact], "exact"
    groups: dict[frozenset, list[dict]] = {}
    for ec, it in usable:
        groups.setdefault(ec, []).append(it)
    keys = list(groups)
    if not keys:
        return None
    seen: set = set()
    for k in keys:
        if seen & k:
            return None
        seen |= k
    if seen != C:
        return None
    return [groups[k] for k in keys], "partition"


def _decide(ub: float, lb: float, claim: dict) -> str:
    bound = _num(claim.get("bound"))
    comp = claim.get("comparator", "at_most")
    if bound is None or not (0 <= bound <= 1):
        return INSUFFICIENT
    if comp == "at_most":
        if ub <= bound:
            return SUPPORTED
        if lb > bound:
            return CONTRADICTED
        return INSUFFICIENT
    if comp == "at_least":
        if lb >= bound:
            return SUPPORTED
        if ub < bound:
            return CONTRADICTED
        return INSUFFICIENT
    return INSUFFICIENT


def _metric_common_checks(claim: dict) -> str | None:
    if claim.get("metric", METRIC) != METRIC:
        return NOT_APPLICABLE
    if claim.get("comparator", "at_most") not in ("at_most", "at_least"):
        return INSUFFICIENT
    b = _num(claim.get("bound"))
    if b is None or not (0 <= b <= 1):
        return INSUFFICIENT
    return None


def _r_metric(claim: dict, items: list[dict]) -> str:
    early = _metric_common_checks(claim)
    if early:
        return early
    alpha = _alpha(claim)
    g = _metric_groups(claim, items)
    if alpha is None or g is None:
        return INSUFFICIENT
    groups, _ = g
    ubs, lbs = [], []
    for grp in groups:
        res = [r for r in (_r_item(it, alpha) for it in grp) if r is not None]
        if not res:
            return INSUFFICIENT
        ubs.append(max(r[0] for r in res))
        lbs.append(min(r[1] for r in res))
    return _decide(max(ubs), min(lbs), claim)


def _s_metric(claim: dict, items: list[dict], key: str) -> str:
    early = _metric_common_checks(claim)
    if early:
        return early
    alpha = _alpha(claim)
    g = _metric_groups(claim, items)
    if alpha is None or g is None:
        return INSUFFICIENT
    groups, _ = g
    a = alpha / len(groups)
    ubs, lbs = [], []
    for grp in groups:
        res = []
        for it in grp:
            r = _s_item(it, a, _seed(key, it.get("id")))
            if r is not None:
                res.append(r)
        if not res:
            return INSUFFICIENT
        top = max(r[2] for r in res)
        best = [r for r in res if r[2] == top]
        ubs.append(max(r[0] for r in best))
        lbs.append(min(r[1] for r in best))
    return _decide(max(ubs), min(lbs), claim)


# ---------------------------------------------------------------------------
# coverage_guarantee
# ---------------------------------------------------------------------------

_LEVEL_RE = re.compile(r"level\s*[=:]?\s*([0-9]*\.?[0-9]+)\s*(%?)", re.I)
_PCT_RE = re.compile(r"([0-9]*\.?[0-9]+)\s*%")


def parse_method_level(method: Any) -> float | None:
    if not isinstance(method, str):
        return None
    m = _LEVEL_RE.search(method)
    if m:
        v, pct = float(m.group(1)), m.group(2)
    else:
        m = _PCT_RE.search(method)
        if not m:
            return None
        v, pct = float(m.group(1)), "%"
    if pct or v > 1:
        v /= 100.0
    return v if 0 < v < 1 else None


def _calibration(items: list[dict], population: str) -> list[dict]:
    return [it for it in items if it.get("kind") == "calibration" and _matches(it, population, False)]


def _cal_info(cal: dict) -> dict | None:
    counts = cal.get("condition_counts")
    cc: dict[str, int] = {}
    if isinstance(counts, dict):
        for k, v in counts.items():
            c = _count(v)
            if not isinstance(k, str) or c is None:
                return None
            cc[k.strip()] = c
    n = _count(cal.get("n"))
    if n is None:
        n = sum(cc.values()) if cc else None
    if n is None or n <= 0:
        return None
    conds = _conds(cal.get("conditions"))
    if conds is None:
        conds = frozenset(k for k, v in cc.items() if v > 0) or None
    if conds is None:
        return None
    if cc:
        conds = frozenset(c for c in conds if cc.get(c, 0) > 0) or frozenset()
    return {"n": n, "conds": conds, "counts": cc, "method": cal.get("method"),
            "dep": cal.get("dependence")}


def _shifts(items: list[dict]) -> tuple[list[tuple[float, float]], bool]:
    """Valid (value, threshold) pairs and whether any linked diagnostic is malformed."""
    out, bad = [], False
    for it in items:
        if it.get("kind") != "shift_diagnostic":
            continue
        v, t = _num(it.get("value")), _num(it.get("threshold"))
        if v is None or t is None or v < 0 or t <= 0:
            bad = True
            continue
        out.append((v, t))
    return out, bad


def _coverage_prelude(claim: dict, items: list[dict]):
    population = _str(claim.get("population"))
    C = _conds(claim.get("conditions"))
    level = _num(claim.get("level"))
    if population is None or C is None or level is None or not (0 < level < 1):
        return INSUFFICIENT
    cals = _calibration(items, population)
    if len(cals) != 1:
        return INSUFFICIENT
    method = cals[0].get("method")
    if isinstance(method, str) and method.strip() and "conformal" not in method.lower():
        return NOT_APPLICABLE
    info = _cal_info(cals[0])
    if info is None:
        return INSUFFICIENT
    ml = parse_method_level(method)
    if ml is None:
        return INSUFFICIENT
    return population, C, level, info, ml


def _r_coverage(claim: dict, items: list[dict]) -> str:
    pre = _coverage_prelude(claim, items)
    if isinstance(pre, str):
        return pre
    _, C, level, info, ml = pre
    n = info["n"]
    same = C == info["conds"]
    if same and level - COVERAGE_SLACK > ml + 1.0 / (n + 1) + 1e-12:
        return CONTRADICTED
    if level > ml + 1e-12:
        return INSUFFICIENT
    if info["dep"] not in ("independent", "clustered", "temporal"):
        return INSUFFICIENT
    if n < 20.0 / (1 - ml):
        return INSUFFICIENT
    shifts, bad = _shifts(items)
    if any(v > t for v, t in shifts):
        return INSUFFICIENT
    if not same and (not shifts or bad):
        return INSUFFICIENT
    return SUPPORTED


def _coverage_dist(info: dict, ml: float, cal: dict | None) -> tuple[float, float] | None:
    n = info["n"]
    D = None
    if cal is not None:
        ok, D = _design_effect(cal, strict=False)
        if not ok:
            return None
    if D is None or D <= 1:
        k = math.ceil((n + 1) * ml - 1e-9)
        if k > n:
            return None
        return float(k), float(n + 1 - k)
    ne = n / D
    if ne + 1 < 1.0 / (1 - ml):
        return None
    return ml * (ne + 1), (1 - ml) * (ne + 1)


def _s_coverage(claim: dict, items: list[dict]) -> str:
    pre = _coverage_prelude(claim, items)
    if isinstance(pre, str):
        return pre
    population, C, level, info, ml = pre
    cal = _calibration(items, population)[0]
    ab = _coverage_dist(info, ml, cal)
    if ab is None:
        return INSUFFICIENT
    a, b = ab
    alpha = _alpha({"confidence": claim.get("confidence", 0.95)}) or 0.05
    target = level - COVERAGE_SLACK
    shifts, bad = _shifts(items)
    if any(v > t for v, t in shifts):
        return INSUFFICIENT
    if C == info["conds"]:
        if stats.beta.cdf(target, a, b) <= S_COVERAGE_RISK:
            return SUPPORTED
        if stats.beta.sf(target, a, b) <= alpha:
            return CONTRADICTED
        return INSUFFICIENT
    if C < info["conds"] and info["counts"]:
        w = sum(info["counts"].get(c, 0) for c in C) / info["n"]
        if w <= 0:
            return INSUFFICIENT
        c_low = stats.beta.ppf(S_COVERAGE_RISK, a, b)
        if 1 - (1 - c_low) / w >= target:
            return SUPPORTED
        # otherwise fall through to the shift-diagnostic route
    if not shifts or bad:
        return INSUFFICIENT
    psi = max(v for v, _ in shifts)
    delta = S_SHIFT_SPREAD * math.sqrt(psi)
    if stats.beta.cdf(target + delta, a, b) <= S_COVERAGE_RISK:
        return SUPPORTED
    return INSUFFICIENT


# ---------------------------------------------------------------------------
# operational_risk
# ---------------------------------------------------------------------------

def _operational_prelude(claim: dict, items: list[dict]):
    population = _str(claim.get("population"))
    recent_pop = _str(claim.get("recent_population"))
    cond = _str(claim.get("condition"))
    bound = _num(claim.get("bound"))
    min_sup = _num(claim.get("min_condition_support"))
    alpha = _alpha(claim)
    if None in (population, recent_pop, cond, bound, min_sup, alpha) or not (0 <= bound <= 1):
        return INSUFFICIENT
    cals = _calibration(items, population)
    if not cals:
        return INSUFFICIENT
    for cal in cals:
        info = _cal_info(cal)
        if info is None or not info["counts"]:
            return INSUFFICIENT
        if info["counts"].get(cond, 0) < min_sup:
            return INSUFFICIENT
    recent = [it for it in items if it.get("kind") in OUTCOME_KINDS and _matches(it, recent_pop)]
    if not recent:
        return INSUFFICIENT
    shifts, bad = _shifts(items)
    if any(v > t for v, t in shifts):
        return INSUFFICIENT
    comp = [it for it in items if it.get("kind") == "completeness"]
    sup = [it for it in items if it.get("kind") == "support_check"]
    return bound, alpha, recent, comp, sup, bad


def _completeness_ok(comp: list[dict], recent: list[dict]) -> bool:
    for c in comp:
        req, av = _num(c.get("required")), _num(c.get("available"))
        if req is None or av is None or av < req:
            return False
        for it in recent:
            n = len(it["outcomes"]) if it.get("kind") == "outcome_records" and isinstance(
                it.get("outcomes"), list) else _count(it.get("n"))
            if n is None or n < req:
                return False
    return True


def _support_ok(sup: list[dict]) -> bool:
    for s in sup:
        v, m = _num(s.get("value")), _num(s.get("support_max"))
        if v is None or m is None or v > m:
            return False
    return True


def _r_operational(claim: dict, items: list[dict]) -> str:
    pre = _operational_prelude(claim, items)
    if isinstance(pre, str):
        return pre
    bound, alpha, recent, comp, sup, bad = pre
    if not comp or not sup or bad:
        return INSUFFICIENT
    if not _completeness_ok(comp, recent) or not _support_ok(sup):
        return INSUFFICIENT
    res = [_r_item(it, alpha) for it in recent]
    if any(r is None for r in res):
        return INSUFFICIENT
    ub, lb = max(r[0] for r in res), min(r[1] for r in res)
    if ub <= bound:
        return SUPPORTED
    if lb > bound:
        return CONTRADICTED
    return INSUFFICIENT


def predictive_bounds(p: float, n_eff: float, alpha: float) -> tuple[float, float]:
    """Beta-binomial predictive bounds for next period's rate (Jeffreys prior)."""
    n_eff = max(n_eff, 1.0)
    k = min(max(p, 0.0), 1.0) * n_eff
    a, b = k + 0.5, n_eff - k + 0.5
    m = max(1, int(round(n_eff)))
    hi = float(stats.betabinom.ppf(1 - alpha, m, a, b)) / m
    lo = float(stats.betabinom.ppf(alpha, m, a, b)) / m
    return hi, lo


def _s_operational(claim: dict, items: list[dict]) -> str:
    pre = _operational_prelude(claim, items)
    if isinstance(pre, str):
        return pre
    bound, alpha, recent, comp, sup, bad = pre
    if comp and not _completeness_ok(comp, recent):
        return INSUFFICIENT
    if sup and not _support_ok(sup):
        return INSUFFICIENT
    for s in sup:
        stat = s.get("statistic")
        if isinstance(stat, str) and "error prob" in stat.lower() and _num(s.get("value")) > bound:
            return INSUFFICIENT
    effs = [_s_effective(it) for it in recent]
    if any(e is None for e in effs):
        return INSUFFICIENT
    top = max(e[2] for e in effs)
    his, los = [], []
    for p, ne, tier in effs:
        if tier != top:
            continue
        hi, lo = predictive_bounds(p, ne, alpha)
        his.append(hi)
        los.append(lo)
    if max(his) <= bound:
        return SUPPORTED
    if min(los) > bound:
        return CONTRADICTED
    return INSUFFICIENT


# ---------------------------------------------------------------------------
# public API
# ---------------------------------------------------------------------------

def _dispatch(assessment: Any, which: str, key: str | None) -> str:
    try:
        up = _unpack(assessment)
        if up is None:
            return INSUFFICIENT
        claim, items = up
        if not items:
            return INSUFFICIENT
        ctype = claim.get("type")
        if key is None:
            key = str(assessment.get("assessment_id", ""))
        if ctype == "metric_bound":
            v = _r_metric(claim, items) if which == "r" else _s_metric(claim, items, key)
        elif ctype == "coverage_guarantee":
            v = _r_coverage(claim, items) if which == "r" else _s_coverage(claim, items)
        elif ctype == "operational_risk":
            v = _r_operational(claim, items) if which == "r" else _s_operational(claim, items)
        else:
            v = INSUFFICIENT
        return v if v in VERDICTS else INSUFFICIENT
    except Exception:  # robustness: never crash on any input
        return INSUFFICIENT


def review_r(assessment: dict) -> str:
    """REFORMS-style checklist reviewer."""
    return _dispatch(assessment, "r", None)


def review_s(assessment: dict, seed_key: str | None = None) -> str:
    """Applied-statistician reviewer.  seed_key (e.g. case_id) fixes resampling."""
    return _dispatch(assessment, "s", seed_key)


def run_cases(cases_doc: Any) -> dict:
    results = []
    cases = cases_doc.get("cases") if isinstance(cases_doc, dict) else None
    for case in cases if isinstance(cases, list) else []:
        cid = case.get("case_id") if isinstance(case, dict) else None
        a = case.get("assessment") if isinstance(case, dict) else None
        key = str(cid) if cid is not None else None
        results.append({"case_id": cid, "r": review_r(a), "s": review_s(a, seed_key=key)})
    return {"schema_version": "triad-m8-comparators-1", "results": results}


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="Run comparator reviewers R and S on a cases file.")
    ap.add_argument("--input", required=True)
    ap.add_argument("--output", required=True)
    args = ap.parse_args(argv)
    with open(args.input, encoding="utf-8") as f:
        doc = json.load(f)
    out = run_cases(doc)
    with open(args.output, "w", encoding="utf-8") as f:
        json.dump(out, f, indent=2)
        f.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
