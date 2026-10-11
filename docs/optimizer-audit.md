# Optimizer convergence is not localization confidence

**Status:** exploratory retrospective analysis of the original, frozen Milestone 2 baseline. This is **not** a new primary hypothesis test and it uses already inspected labels.

## What the recorded data say

The saved `results/localization.csv` contains 1,080 test fits spanning ten scenarios (108 predictions per scenario). Their baseline estimator always returns a location.

| Stopping flag | Predictions | Errors >5 mm | Mean position error |
| :--- | ---: | ---: | ---: |
| Did not meet convergence criterion | **451** | **10** | **1.307 mm** |
| Marked converged | **629** | **10** | **1.144 mm** |
| All predictions | **1,080** | **20** | approximately **1.212 mm** |

The exact source data are committed; the CLI `triad audit` recalculates these conditional metrics from `results/evaluation.json` and records that file's SHA-256. Each figure is descriptive and conditioned on the existing synthetic design.

**Ten of twenty large errors occurred in fits marked converged.** An algorithmic stop flag should not be sold as a calibrated reliability or physical-observability indicator. Likewise, a nonconvergence flag does not prove a bad estimate.

Some scenario-specific contrasts:

- Strong drift: 41/108 flagged, but only two >5 mm errors, divided one flagged and one converged.
- Combined drift/material mismatch: 42/108 flagged, six >5 mm errors, **four converged**.
- Weak-sensitivity region: 55/108 flagged, one >5 mm error, **converged**.
- Near-ambiguity pool: 66/108 flagged, two >5 mm errors, one flagged and one converged.

This does **not** establish that convergence flags lack predictive value everywhere: event counts are too low for such a general claim. It establishes that relying on the flag alone would miss important failures in this pilot.

## Reproduce without altering the baseline

```sh
cargo run --locked -- audit
```

Reads the saved `results/evaluation.json` and writes only `results/optimizer-audit.json`. The report classifies fits by convergence flag and scenario, computes means and over-5-mm counts, records a source digest, and states its own limitations.

The audit does not run the solver, touch test thresholds, fit a rejection rule, or make a scientific claim about Method D.

## What deserves engineering attention

1. **Verify termination independently.** Investigate projected steps at parameter bounds, damping escalation, piecewise multilinear interpolation knots, and fixed 100-iteration budget. A valid constrained solution may fail an unconstrained-like stopping criterion.
2. **Check objective quality rather than status alone.** Compare the returned residual with the best sampled dictionary state and an independently optimized development-set solution. Assess whether some flagged solutions are equally good, or substantially worse, in objective value.
3. **Separate error sources.** Evaluate continuous refined-FEM truth versus coarse-mesh FEM and dictionary interpolation. Compare mismatch with the full measurement-noise covariance.
4. **Respect the frozen experiment.** Do not alter the prior test set or thresholds to make the fixed baseline look better. Any new optimizer becomes a *new method* requiring a revised preregistration and fresh test sequences.
5. **Preserve the strongest Bayesian comparator.** An optimizer error detector is not the same as a physical ambiguity detector. Method D must outperform posterior expected-error rejection (C2) at matched acceptance coverage, not merely veto unconverged fits.

Read [the research status](research-status.md), [model limitations](limitations.md), and [next experiment](next-experiment.md).
