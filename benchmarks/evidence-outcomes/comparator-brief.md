# Comparator brief: reviewing reliability claims against evidence

This is the **only** document given to whoever implements the comparators **R** and
**S** for Milestone 8. It describes the input format, what each claim asserts, and
how reviews are scored. It deliberately says nothing about how any other reviewer
decides, or how the cases are generated.

## Your task

Write two independent reviewers, as Python functions, that read one *assessment*
(a reliability claim plus its evidence) and return one of four verdicts for the
claim:

- `supported_within_scope`: the evidence is sufficient to accept the claim as
  stated, at its stated confidence;
- `insufficient_evidence`: the evidence does not establish the claim either way;
- `contradicted`: the evidence shows the claim is false;
- `not_applicable`: the claim does not make sense for this system.

Only `supported_within_scope` counts as endorsing the claim. The other three all
count as not endorsing it.

- **R, a checklist reviewer.** Model it on the REFORMS recommendations for
  ML-based science (Kapoor et al., *Science Advances* 10:eadk3452, 2024,
  https://doi.org/10.1126/sciadv.adk3452). Items cover representativeness of the
  evaluation sample for the population the claim is about, uncertainty
  quantification that respects how the data were collected, appropriate
  comparisons, and limits to generalisation. Your R may encode explicit scope and
  dependence rules. Make it as strong as you can.
- **S, a statistical reviewer.** A careful applied statistician who uses the most
  appropriate inference the evidence allows: exact binomial bounds,
  dependence-aware intervals, design effects, block or cluster resampling where
  the raw outcomes permit it, conformal-prediction theory, and shift diagnostics.
  Make it as strong as you can.

R and S should be genuinely different reviewers. Neither may look at anything
except the assessment passed to it.

## How the reviews will be scored

Every claim is a statement about a real, finite population of predictions made by
a fixed model: for example, all predictions in a twelve-month period, or in one
month, batch or week. After you deliver your code, it will be run once on cases
built from real public data. Each claim is then labelled **true** or **false**
from the actual outcomes of that population, which no reviewer ever sees.

- A reviewer **pays 1** for each false claim it endorses.
- It **pays 0.25** for each true claim it does not endorse.
- Costs are averaged at equal weight for true and false claims within each of
  several claim families, then across families.
- Claims are stated at 95% confidence. A well-behaved reviewer should endorse a
  false claim rarely, at most about 5% of the time.

A reviewer that never endorses anything pays 0.125 under this scoring. A useful
reviewer must do better than that, by endorsing many true claims and few false
ones.

## Input format

Your functions receive one assessment, a JSON object (Python `dict`):

```json
{
  "schema_version": "triad-evidence-1",
  "assessment_id": "...",
  "evidence": [ {...}, ... ],
  "claims": [ {...} ]
}
```

There is exactly one claim. Its `evidence` field lists the ids of the evidence
items linked to it; consider only those. The CLI wrapper receives a cases file
instead:

```json
{"schema_version": "triad-evidence-outcomes-cases-1", "cases": [{"case_id": "...", "assessment": {...}}]}
```

### Shared vocabulary

- `population`: names the model and metric the evidence or claim concerns.
- `conditions`: a list of stratum labels (for example months such as `2014-09`,
  batches such as `batch5`, or contact months such as `2009-07`). For **evidence**,
  these are the strata its units actually cover. For a **claim**, they are the
  strata the claim is about.
- `metric`: always `material_error_rate`, the fraction of predictions whose error
  is unacceptably large (a misclassification, or a regression error beyond a
  declared tolerance).
- `unit`: what one outcome is (hour, site-hour, measurement, contact).
- `dependence`, which describes how the evidence units were collected:
  - `independent`: a simple random sample of units from the claimed population;
  - `clustered`: whole clusters (for example calendar days, or blocks of
    consecutive records) sampled at random, with every unit in each;
  - `temporal`: one or more whole contiguous periods (units are consecutive in
    time and may be autocorrelated);
  - `unknown`: not stated.

### Evidence kinds you will encounter

`outcome_records`: raw outcomes, in time (or archive) order.

```json
{"kind": "outcome_records", "id": "...", "metric": "material_error_rate", "population": "...",
 "conditions": ["..."], "unit": "hour", "dependence": "clustered",
 "outcomes": [false, true, ...], "design_effect": null}
```

`true` means a material error occurred. `design_effect`, when not null, is a
variance-inflation factor D ≥ 1 supplied by the evidence author: the effective
sample size is n/D. Cluster boundaries are **not** included.

`metric_report`: summary counts as reported by the evidence author.

```json
{"kind": "metric_report", "id": "...", "metric": "material_error_rate", "population": "...",
 "conditions": ["..."], "unit": "hour", "n": 1234, "events": 98, "value": null,
 "dependence": "temporal", "design_effect": null, "source": "...",
 "interval": {"method": "block_bootstrap", "level": 0.95, "lower": 0.061, "upper": 0.097}}
```

`events` is the number of material errors among `n` units. `interval` may be
null. When present it is a **two-sided** interval at `level` for the error rate.
Its `method` is one of `clopper_pearson`, `wilson`, `normal_iid` (all assume
independent units), `block_bootstrap` (moving-block bootstrap over time) or
`cluster_bootstrap` (resampling whole clusters).

`calibration`: describes a calibration set used for split-conformal prediction
intervals.

```json
{"kind": "calibration", "id": "...", "population": "...", "conditions": ["2014-09"],
 "condition_counts": {"2014-09": 640}, "n": 640, "unit": "hour", "dependence": "temporal",
 "method": "split conformal, absolute log1p residual, level 0.90"}
```

`condition_counts` gives the number of calibration units in each stratum. In
operational claims (below), the calibration set is the model's development
period, and its `condition_counts` are by calendar month.

`shift_diagnostic`: a label-free comparison of the model's predictions (not
outcomes) between a reference set and the target set.

```json
{"kind": "shift_diagnostic", "id": "...", "reference": "...", "target": "...",
 "statistic": "population stability index of predictions (10 reference-decile bins)",
 "value": 0.31, "threshold": 0.25}
```

The evidence author treats `value > threshold` as a material shift.

`support_check`: a monitoring statistic for the recent period, compared with the
largest value of the same statistic seen during calibration.

```json
{"kind": "support_check", "id": "...", "statistic": "mean predicted error probability, previous week",
 "value": 0.12, "support_max": 0.10}
```

`completeness`: how many labelled recent units were available, against how many
the evidence author required.

```json
{"kind": "completeness", "id": "...", "required": 152, "available": 168}
```

### Claim types you will encounter

All carry `id`, `statement` (free text, for humans only), `evidence` (linked ids)
and `confidence` (0.95).

`metric_bound`: "the material-error rate of `population` over the strata in
`conditions` is at most `bound`, asserted at 95% confidence".

```json
{"type": "metric_bound", "id": "claim", "statement": "...", "evidence": ["..."],
 "metric": "material_error_rate", "population": "...", "conditions": ["2014-09", "..."],
 "comparator": "at_most", "bound": 0.0812, "confidence": 0.95,
 "claimed_value": null, "tolerance": 0.0}
```

The true value is the actual rate over **all** units of the named strata.

`coverage_guarantee`: "split-conformal prediction intervals calibrated as
described cover at least a fraction `level` of outcomes in the population and
strata named by the claim".

```json
{"type": "coverage_guarantee", "id": "claim", "statement": "...", "evidence": ["..."],
 "population": "...", "conditions": ["2014-10"], "level": 0.9}
```

The claim is judged true if the realised coverage over the target units is at
least `level − 0.02`.

`operational_risk`: "the material-error rate over the coming week will be at most
`bound`". Here `population` names the calibration evidence and `recent_population`
the recent outcome evidence. `condition` is the calendar month the coming week
falls in. `min_condition_support` is the evidence author's own requirement for
the number of calibration units in that month.

```json
{"type": "operational_risk", "id": "claim", "statement": "...", "evidence": ["..."],
 "population": "...", "recent_population": "...", "condition": "2014-11",
 "bound": 0.21, "confidence": 0.95, "min_condition_support": 100}
```

The claim is true if the coming week's realised rate is at most `bound`. The
recent outcomes are from the previous week.

## Deliverable

A single file `m8_comparators.py`:

- `review_r(assessment: dict) -> str` and `review_s(assessment: dict) -> str`,
  each returning one of the four verdict strings for the assessment's single claim.
- A CLI: `python m8_comparators.py --input CASES.json --output OUT.json`. It writes
  `{"schema_version": "triad-m8-comparators-1", "results": [{"case_id": ..., "r": ..., "s": ...}]}`
  in input order.
- It must be deterministic. Any resampling uses a fixed seed derived from the
  `case_id`, for example with `numpy.random.default_rng`.
- Dependencies: the Python standard library, NumPy 2.2.6 and SciPy 1.15.3 only.
- A module docstring stating, for each reviewer, its decision rules in plain words.
- Tests in `test_m8_comparators.py` (pytest-style, plain asserts) using small
  synthetic assessments that you write yourself.

Do not tune anything to real data; you will not receive any.
