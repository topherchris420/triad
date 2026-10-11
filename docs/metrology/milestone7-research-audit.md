# Milestone 7 research audit

Vers3Dynamics, PI Christopher Woodyard. Audit performed October 11, 2026, before
any Milestone 7 code, protocol or model was written.

## Starting point

- `master` fetched at `ba2815f111c670b2428790c3882898b8c4f29d28`; the development
  branch `claude/tender-bardeen-hof9f3` was identical to it.
- The master CI run for that commit,
  [Rust verification 38176442642](https://github.com/topherchris420/triad/actions/runs/38176442642),
  completed successfully on the `ubuntu-latest` and `windows-latest` matrix.
- Locally (Linux x86-64, rustc 1.97.0), `cargo test --release --locked` passed every
  test, and `metrology validate` replayed the archived Milestone 6 report
  `results/benchmark-electricity/report.json` exactly.
- The official NIST submission repository was re-cloned at
  `7f0609217621aa905973ad0cc03f5e6a8c97e345` (the same commit recorded in
  Milestone 6). Its `validation/schemas/v1.json` is byte-identical to the vendored
  `nist-schema-v1.json` (SHA-256 `f815015d…a7c`).

## What exists

| Component | Source | What it computes | Verification status |
|---|---|---|---|
| Generic selective-risk evaluator | `src/metrology.rs` | Calibration-score quantile threshold, coverage, conditional excess-loss risk, tie-aware risk–coverage curve and AURC, event Brier/ECE, whole-cluster bootstrap, validity annotations | Rust unit tests, exact replay, independent standard-library Python check (M5) |
| Ordered-stream benchmark | `src/metrology_benchmark.rs` | Binary-classification temporal windows, label-free warning scores, calibration-quantile alerts, degraded-window confusion, paired circular moving-block bootstrap | Rust tests, exact replay, independent NumPy check (M6), clean retraining check |
| Conservative abstention controller | `src/abstention.rs` | Fixed-candidate threshold with simultaneous Hoeffding bounds over independent equal-length sequences | Rust tests; synthetic fixture only |
| Capture inspector | `src/capture.rs` | Ten-channel recording schema and empty-reference drift/covariance diagnostics | Rust tests; synthetic fixture only |
| 2D capacitive physics, inverse estimator, C2/D selectors | `src/forward.rs`, `src/inverse.rs`, `src/selective.rs` and related | FEM forward model, multistart GLS inversion, posterior and physics-informed rejection scores | Numerical gates **failed** (M3/M4/M5); scientific hypothesis **not supported** (M3) |
| Append-only research artifacts | `src/research_integrity.rs` | Exclusive file creation; frozen-protocol checks | Rust tests |
| Historical hash manifests | `docs/metrology/baseline-hashes.json` (62 files), `benchmarks/electricity/frozen-before-v6*.json` (73 files, local and canonical Git bytes) | Byte immutability of earlier evidence | Rust test and NumPy verifier |
| NIST draft tooling | `docs/metrology/nist-submission-draft.yml`, `scripts/validate-nist-draft.py` | Schema-format validity of a draft | CI; format only |

Important architectural constraint discovered in this audit: every generic report
embeds `evaluator_source_sha256` of `src/metrology.rs` and the SHA-256 of
`Cargo.lock`; every benchmark report embeds the hash of
`src/metrology_benchmark.rs`. Archived reports are validated by exact replay, so
**editing either source file or adding any crate dependency would make the
archived Milestone 5 and Milestone 6 evidence fail replay.** Milestone 7 therefore
adds new modules, reuses the existing public functions, and adds no dependency.

## Established, engineered, hypothesised

| Claim | Status | Basis |
|---|---|---|
| Selective risk and coverage, risk–coverage curves | **Scientifically established** in the literature (El-Yaniv & Wiener 2010; Geifman & El-Yaniv 2017), not by TRIAD | TRIAD implements them |
| Binned calibration diagnostics (ECE, Brier) | **Established** (Brier 1950; Guo et al. 2017), with documented limitations | Implemented for the excessive-error event |
| Block bootstrap for dependent series | **Established** for stationary series (Künsch 1989) | Implemented as sensitivity analysis; stationarity unverified |
| Split conformal intervals and their exchangeability requirement | **Established** (Vovk et al. 2005; Lei et al. 2018) | Not yet implemented in TRIAD before M7 |
| Calibration-fixed thresholds, exact deterministic replay, strict schemas, append-only artifacts | **Engineering implementation**, computationally verified | Does not establish scientific utility |
| Validity annotations (`numerical_status`, `scientifically_unsupported`, …) | **Engineering implementation**; flags are copied from user attestations and are not verified | M6: changing them altered no decision |
| Label-free combined covariate/confidence warnings add predictive value | **Hypothesis, not supported** on Electricity (M6): no added detections, more false alarms | Immutable negative finding |
| Window gating improves conditional risk | **Hypothesis, not established** (M6): large coverage loss, intervals include zero | Immutable negative finding |
| Physics-informed selector D beats posterior C2 | **Hypothesis, not supported** (M3): 0.174 pp reduction versus 2 pp threshold; paired interval [0.035, 0.382] pp | Immutable negative finding |
| 2D forward model adequate for likelihood-based confidence | **Contradicted** at the declared inspection level (M3 surrogate discrepancy 37.78 noise units; M4/M5 mesh/boundary discrepancies) | Immutable negative finding |
| TRIAD can classify whether evidence supports a specific reliability claim better than standard checks | **Open hypothesis**, tested for the first time in M7 | — |

## Five concepts that must not be conflated

1. **Measurement validity** — whether a measured quantity represents what it is
   claimed to represent (correct loss, population, label definition, units,
   numerical adequacy of any simulator). A perfectly reproducible metric can be
   invalid for the stated purpose.
2. **Prediction reliability** — the frequency of unacceptable errors among
   predictions that are actually used, at a stated coverage, on a stated
   population. Selective risk is one operationalisation.
3. **Distribution shift** — a difference between the populations that produced
   training/calibration evidence and the population of use. Observable covariate
   or score changes do not identify a change in P(Y|X); seasonal change is not
   automatically concept drift.
4. **Uncertainty estimation** — per-prediction scores or intervals (e.g. learned
   error scale, conformal intervals) and population-level metric uncertainty
   (confidence intervals for risk). They answer different questions and rest on
   different assumptions (exchangeability, independence, stationarity).
5. **Evidence sufficiency** — whether the available, verifiable evidence is enough
   to justify a *specific claim* at its stated scope. It is relative to the claim:
   the same evidence can support "coverage was 80% on this test period" and fail
   to support "coverage will be 80% next winter". Before M7, TRIAD reported
   evidence metadata but never evaluated a claim against it.

## Weaknesses carried into Milestone 7

- Electricity chronology is unresolved and its upstream normalisation cannot be
  reversed; temporal interpretation there is conditional.
- Each split is one dependent series; three/one degraded windows per model cannot
  support warning-utility inference.
- Warnings were end-of-window batch decisions, not prospective.
- Validity flags are unverified attestations that do not enter any decision.
- The benchmark contract supports binary classification only.
- No baseline for "standard evidence checks" exists, so evidence-related claims
  of value have never been compared against anything.
- All independent verification so far is independent *code*, not independent
  *investigators*.

## Implications for the Milestone 7 design

- Add a second real dataset with verifiable timestamps (UCI Bike Sharing, hourly),
  a regression task, and an explicit missing-hour policy.
- Make warnings prospective (decided before the period they govern) and compare
  them with label-based monitoring, which is legitimately available when labels
  arrive with a short delay.
- Formalise claims and evidence with explicit, inspectable rules; separate
  computed facts, supplied numbers and attestations; never let unknown evidence
  pass.
- Compare against a deliberately strong "standard evidence checks" baseline and
  report operational equivalence where it holds.
- Freeze the protocol and the claim-validation suite (with ground truth) in Git
  before fitting models or running classifiers.
- Do not modify any frozen file; add a new hash manifest covering Milestone 6
  outputs.
