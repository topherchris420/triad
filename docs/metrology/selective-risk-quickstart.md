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

## Where it does and does not apply

It applies to any model that outputs a per-instance score and can defer, given
labelled calibration and test data. It does **not** measure probability calibration
(use Brier score or ECE alongside it). It does not certify risk under future
distribution shift. It does not replace a refusal-rate measurement for generative
systems, where abstention is a policy decision rather than an error-control
mechanism. A lower selective risk obtained by abstaining more is not an
improvement unless coverage is matched or the cost of abstention is stated.
