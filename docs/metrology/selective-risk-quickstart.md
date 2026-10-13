# Selective risk at target coverage: quickstart

This page shows how to compute the established selective-risk metric with TRIAD.
The metric is due to the selective-classification literature (El-Yaniv and Wiener
2010; Geifman and El-Yaniv 2017). TRIAD provides one implementation of it and
claims no novelty for the metric.

## Definition

A selective predictor `(f, g)` answers with `f(x)` when its selection function
accepts the input, `g(x) = 1`, and abstains otherwise. TRIAD uses a score `s`
(smaller means more confident) and a threshold `t`, so `g(x) = 1{s(x) <= t}`.
For a loss bounded in `[0, 1]`:

- **coverage** `phi = E[g(X)]`, the fraction of inputs answered;
- **selective risk** `R = E[loss * g(X)] / phi`, the mean loss on answered inputs.
  It is undefined when nothing is accepted.

TRIAD uses the indicator loss `1{L(y_hat, y) > epsilon}`. For classification this
is the 0/1 error (`loss: "classification"`, `tolerance: 0`). For regression it is a
material-error event, `|y_hat - y| > epsilon`, using `loss: "euclidean"` with a
declared `tolerance`.

## Fixing the threshold at a target coverage `c`

1. Use a **calibration split** that is disjoint from the test split. Sort its scores
   and set `t` to the `ceil(n*c)`-th smallest one. Calibration labels are not used,
   and every tie at `t` is accepted.
2. On the **test split**, report the realised coverage, the selective risk, the
   accepted count and the accepted-error count.
3. If test units are dependent (time, sites, patients), report an interval that
   resamples whole sequences or clusters, not individual rows.

The realised test coverage will differ from `c`. The calibration quantile gives no
finite-sample guarantee of coverage or risk, especially under distribution shift.

The **retrospective** variant ranks test scores and accepts exactly `floor(n*c)`
test rows. It suits comparing models at matched coverage, as do the risk–coverage
curve and its area (AURC), but it uses the test set and is not a deployable
threshold.

## Run it

```sh
cargo run --release --locked -- metrology evaluate --help
cargo run --release --locked -- metrology evaluate --input examples/predictions.json --output my-report.json
```

The input schema is `triad-metrology-1` ([example](../../examples/predictions.json)).
Each record carries `prediction`, `truth`, `uncertainty` (the score `s`), an optional
`error_probability`, `split` (`calibration` or `test`), `scenario` and a `sequence`
identifier for resampling. In the report, `report.overall.operational` holds
`coverage`, `selective_error_risk`, `accepted` and `accepted_errors`.
`report.overall.retrospective_matched_coverage` and `risk_coverage_curve` hold the
descriptive variants. `sequence_bootstrap` and `spatial_bootstrap` hold the
resampling intervals.

Precise conventions (tie handling, AURC as a right-endpoint step integral,
bootstrap details) are in [method-definition.md](method-definition.md).
[milestone7-reproduction.md](milestone7-reproduction.md) gives a full real-data
example: hourly UCI Bike Sharing, regression, calibration-fixed threshold,
moving-block bootstrap.

## Without TRIAD: a Python reference

The calibration-fixed form needs only a sort. This standard-library function
follows the same conventions as `metrology evaluate`. CI runs
[`scripts/check-quickstart-reference.py`](../../scripts/check-quickstart-reference.py),
which executes the block below and requires it to match TRIAD exactly on the example
and on randomized inputs with score ties, both loss types, and `c = 0` and `c = 1`.

```python
import math

def selective_risk_at_coverage(cal_scores, test_scores, test_errors, c):
    """Selective risk at target coverage c (El-Yaniv & Wiener 2010).

    cal_scores   uncertainty scores on a calibration split (smaller = more confident)
    test_scores  scores on a disjoint test split
    test_errors  per test row, 1 if the loss event occurred, else 0
                 (classification: y_hat != y; regression: |y_hat - y| > epsilon)
    c            target coverage in [0, 1]
    """
    if not 0.0 <= c <= 1.0:
        raise ValueError("c must be in [0, 1]")
    ranked = sorted(cal_scores)
    k = math.ceil(c * len(ranked))
    t = ranked[k - 1] if k > 0 else None              # c = 0 rejects every input
    accept = [t is not None and s <= t for s in test_scores]  # ties at t accepted
    accepted = sum(accept)
    accepted_errors = sum(1 for a, e in zip(accept, test_errors) if a and e)
    return {
        "threshold": t,
        "coverage": accepted / len(test_scores),      # realised, not the target c
        "selective_risk": accepted_errors / accepted if accepted else None,
        "accepted": accepted,
        "accepted_errors": accepted_errors,
    }
```

For example, with a classifier's labels and scores already split:

```python
result = selective_risk_at_coverage(
    cal_scores, test_scores,
    [int(p != y) for p, y in zip(test_pred, test_true)],
    c=0.8,
)
```

Report `coverage`, `selective_risk` and both counts together. For an interval,
use a binomial interval (for example Clopper-Pearson) on
`accepted_errors / accepted` when test units are independent, or resample whole
sequences or clusters when they are not.

## Where it does and does not apply

It applies to any model that outputs a per-instance score and can defer, given
labelled calibration and test data. It does **not** measure probability calibration
(use Brier score or ECE alongside it). It does not certify risk under future
distribution shift. It does not replace a refusal-rate measurement for generative
systems, where abstention is a policy decision rather than an error-control
mechanism. A lower selective risk obtained by abstaining more is not an
improvement unless coverage is matched or the cost of abstention is stated.
