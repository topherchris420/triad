# Electricity external-data benchmark

The Rust evaluator consumes `manifest.json`; Python is needed only for dataset retrieval, fitting and independent checks. The manifest includes the existing metrology input schema for each model, original zero-based row IDs, disjoint calibration/test records, explicit ordered windows, model hashes, source provenance and protocol hash. Each record's class prediction and error probability recover the UP probability (`1-error_probability` when prediction is UP, otherwise `error_probability`). All features and trained tree/linear coefficients are documented and exported. The full training/development inputs are retrieved, not redistributed.

The committed protocol precedes fitting in commit `3f3046f`. This is local exploratory prespecification, not third-party registration. `provenance.json` records data and feature hashes, exact split counts, scaler parameters, software versions and source preprocessing. `metadata.json` and `features.json` preserve OpenML's authoritative responses. Raw ARFF data are not included. The metadata calls its license **Public**, without naming a specific legal license; do not reinterpret that as CC0 or MIT. The repository's MIT license covers our code, not a relicensing of source observations.

## Reproduce the recorded measurements

```sh
cargo run --release --locked -- metrology benchmark --help
cargo run --release --locked -- metrology benchmark --input benchmarks/electricity/manifest.json --output results/my-electricity
cargo run --release --locked -- metrology validate --input results/my-electricity/report.json
python -m pip install -r benchmarks/electricity/requirements.txt
python scripts/verify-benchmark.py results/my-electricity/report.json --plots results/my-electricity/plots
```

Use Python 3.11 or 3.12. Versions are pinned; platform-level BLAS rounding can differ. Retrain from the exact source data into **new** directories:

```sh
python scripts/prepare-electricity.py --cache work/electricity-cache --output work/electricity-retrained
python scripts/check-electricity-reproduction.py work/electricity-retrained
```

The preparation script verifies the SHA-256/MD5, source version, feature schema, finite values, period/weekday ordering and disjoint day splits. It rejects replacement data rather than changing the protocol. It fits only on training rows and fits its scaler there. The exact forecast task uses known current period/weekday and prior-day price/demand/transfer. No current price (which constructs the class) or current demand/transfer enters the model. No hyperparameter search or test-driven calibration takes place. Upstream global normalization and missing-value replacement cannot be undone and prevent a strong leakage-free deployment claim.

## Contract and uncertainty

`triad-benchmark-1` intentionally supports binary classification with complete ordered day blocks; other modalities still use the generic `metrology evaluate`. It rejects unknown typed fields, nonbinary labels, invalid scores/hashes, crossed or missing windows, nonconsecutive row IDs, model sample mismatch and unsupported assurance claims. `provenance` is descriptive metadata, not a certificate. Hash syntax/replay alone cannot authenticate data, model identity or absence of test leakage: the preparation/retraining audit is separate. Each split is one dependent sequence, so the generic independent-sequence interval is unavailable. Temporal intervals are supplied separately.

Diagnostics are label-free standardized covariate mean changes and confidence-distribution mean changes relative to development. Neither is a comprehensive two-sample test. Calibration windows determine each alert threshold at the frozen 80th percentile; test labels define retrospective degradation only. No recomputation of threshold with test data is allowed. Warnings and gating occur at window end, before labels, so delay 0 means the first degraded window's end, **not advance warning**. Degraded episodes missed entirely have null delay.

Bootstrap resamples circular contiguous blocks of 1, 7 and 14 published day units, not individual rows. The deterministic SplitMix64 draws with rejection and linear percentile interpolation are independently implemented in NumPy. Shared sampled days support paired comparisons; thresholds/models are held fixed. This does not cover fitting uncertainty, upstream preprocessing, calendar uncertainty or arbitrary nonstationarity. Single-window 7/14-day intervals are null because fewer than two full blocks are available. No degenerate zero-event percentile interval is offered as evidence of safety. Missing truth makes the corresponding risk/utility unknown. Coverage remains observable.

Simple combined-warning gating is exactly the TRIAD gate, so the extra metadata contributes transparency only. Covariate-only and confidence-only policies are ablations, each calibrated to the same nominal alert budget. Actual test budgets may differ and are reported. Numerical and statistical context flags are constant here and have no warning discrimination; adding or removing those annotations leaves all decisions unchanged. This benchmark cannot test the scientific utility of physical numerical validity because neither model has a numerical physics component.

Read [executed results](../../docs/metrology/milestone6-real-world-results.md) before interpreting the figures.
