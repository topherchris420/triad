# TRIAD Research Milestone 3: Selective Prediction and Geometry Screening

**State:** implementation committed; new study results NOT yet established. The Milestone 2 baseline is frozen and must not be silently overwritten.

## The new reproducible commands

```sh
cargo test --locked
cargo run --release --locked -- numerical
cargo run --release --locked -- selective
cargo run --release --locked -- geometry-screen
```

The first command tests numerical code, not scientific accuracy. The later commands are deliberately **not** part of `triad all`; they can require long CPU runs. They write fresh versioned artifacts and refuse to overwrite existing reports. The selective command also refuses an existing run directory, including incomplete runs. Read [research integrity](research-integrity.md) before running or recovering a study.

### 1. Numerical audit (`numerical`)

`src/numerical_audit.rs` checks six new, off-grid synthetic development states. It compares 1 mm and 0.5 mm FEM signals in **full covariance-whitened units**, measures coarse-dictionary interpolation mismatch against a refined continuous solve, and compares the fitted optimizer objective with the best discrete candidate.

Output: `results/numerical-audit-v3.json`. An inspection criterion of at most one whitened noise unit for mesh and interpolation mismatch plus a near-zero excess optimizer objective is a **provisional gate**, not proof that the likelihood is correctly specified. This study may expose a serious model discrepancy. Do not modify the old experimental config or retroactively claim old estimates are calibrated.

### 2. Predeclared selective test (`selective`)

See the immutable-by-process versioned protocol in `experiments/selective-v3.json`. It uses the **same ten capacitance channels, frozen conventional point estimator, calibration budget, prior and finite-dictionary likelihood** for both methods.

- **C2:** approximate posterior mass of positions farther than 5 mm from the common reported point. The posterior is a finite-grid, equal-state-prior, temperature-2 Gaussian approximation; its probability values are not claimed to be calibrated until tested.
- **D:** a frozen 75% C2 and 25% physical uncertainty mixture. The physical term equally combines the nuisance-adjusted weakest local Fisher-information direction and the posterior-average dictionary global near-collision surrogate. Observability is computed from estimated states or the posterior, **never the hidden true state**.
- **Probability calibration:** separate monotone logistic calibration maps are fitted exclusively on new development labels. Independent test Brier scores, equal-width reliability bins and ECE inspect exceedance probability estimation.
- **Selection calibration:** raw-score cutoffs are set only on new calibration sequences for nominal 80% deployment coverage. Shifted **actual** test coverage and conditional risk are reported.
- **Primary retrospective contrast:** exact matched 80% test coverage by unlabeled score ranking, with C2-minus-D conditional error risk and a paired whole-sequence bootstrap interval. This is a **ranking comparison**, not a deployable threshold.
- **Statistical caution:** 64 development, 64 calibration and 300 independent test sequences, each with 12 frames under the fixed strong-drift generator. Each split gets 24 new independently sampled off-grid spatial/material sites and independent observation streams. Positions are **shared within** a split and frames within a sequence are dependent. Whole-sequence bootstrap handles the latter, not all generalization uncertainty.

Output: `results/selective-v3/test-scores.json` (written before truth joins), `results/selective-v3/evaluation.json` and `results/selective-v3/summary.json`. The primary hypothesis can have supportive *exploratory* evidence only when the numerical gate passes, at least 30 final errors occur, risk reduction reaches 2 percentage points and the paired bootstrap lower bound excludes zero. Even so, independent scientific replication remains necessary.

Why D might fail: a correctly specified posterior C2 is Bayes-optimal for this tolerance loss. The only credible rationale for improvement is robustness to approximate inference, nuisance modeling, or environmental shift. If D loses, the negative result is valuable.

### 3. Electrode geometry screening (`geometry-screen`)

Only available after a completed `selective-v3/summary.json` exists, matches the current protocol, and its run, score and evaluation files pass SHA-256 checks. The forward solver's **existing default electrode centers and strip width remain unchanged**. Alternate layouts are explicitly parameterized via `ElectrodeLayout`, validating strip ordering, boundary clearance and overlap.

Five equal-width, four-electrode candidates compete under identical synthetic 2D FEM discretization, channel count, noise covariance, material range and region of interest. Selection uses the 10th percentile of whitened pairwise signal separation for states more than 5 mm apart on development sites. Independently drawn holdout sites are used **only after** selecting the candidate. The outcome is a **screening surrogate**, not a demonstrated reduction in selective risk or a manufacturable sensor design.

Output: `results/geometry-screen-v1.json`. To claim a genuine second research result, the selected layout must next receive its own calibrated inference model, numerical mesh checks, power calculation, independent holdout test sequences, and equivalent hardware/3D validation when making physical-device claims.

## Integrity and publication restrictions

1. Never overwrite or refit the frozen Milestone 2 dataset, predictions, validated protocol, and plots to make new methods look better.
2. All new test labels are isolated from model-fitting, posterior scoring, physics features and acceptance thresholds.
3. Fix the loss at 5 mm, the matched test coverage at 80%, and the declared practical improvement at **two absolute percentage points** before looking at results.
4. Do not conflate precise p-values or narrow frame-level intervals with independent-sequence evidence.
5. Don't market improvement, sensor superiority, or an invention from unrun code, low failure-event counts, a noisy surrogate, or a single simulation.

**Vers3Dynamics principle:** physical measurements constrain inference; calibrated uncertainty constrains what the system may responsibly claim.
