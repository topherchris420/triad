# Research status: Milestones 0–2

**Vers3Dynamics** · **Principal investigator Christopher Woodyard** · 2026-10-11.
*Resonance as Substrate. Intelligence as a Layer.*

Milestones 0, 1 and 2 are complete as a bounded computational prototype, entirely
in **Rust**, with a recorded optimization limitation. The proposed physics-informed
selective prediction hypothesis remains **untested**. No evidence establishes
novelty, improvement over posterior expected-error rejection, or superiority to
commercial CAPSENSE. No selective method, electrode optimization, hardware or
firmware experiment was implemented.

## Commercial references and REA

The pinned CAPSENSE source audit covers acquisition/control, baseline tracking
and resets, raw-count filters, threshold dispatch, noise-envelope/common-mode
contracts, proximity, hysteresis/debounce, slider/touchpad processing, position
filtering and fault/status checks. Some advanced slider bodies are readable;
adaptive filtering, noise-envelope/common-mode and advanced touchpad inner
algorithms remain unverified library implementations. Six precompiled archives
were inventoried by repository metadata, not downloaded/decompiled/executed.
See [architecture audit](commercial-reference-analysis.md) and
[algorithm evidence](algorithm-evidence.md). User-reported Rust permission is
recorded separately from the published license. No commercial code is included.

REA's documented `binary_session({})` actually responded: rea-agents 6.4.0,
170 tools, no target open and no analysis observed. Ghidra was unavailable under
the connected ARM64 Windows host/configuration; Hopper unsupported and IDA
unconfigured. This is a verified MCP capability audit, **not a binary analysis**.
[Audit](rea-capability-audit.md), [returned evidence](../results/rea-session-evidence.json).

## Physical validation: passed

Independent mathematical P1 FEM assembled by Fenris, with Spade interface meshes
and faer sparse solves. Fixed four-strip geometry; grounded finite enclosure;
circular dielectric-cylinder cross-section. Ten signed self/mutual Maxwell
capacitance channels, baseline-subtracted, in **pF/m**.

| Check | Actual worst value | Frozen gate |
|---|---:|---:|
| Total capacitance, 1 to 0.5 mm mesh | 1.956% | 4% |
| Target-induced signal, 1 to 0.5 mm | 3.851% | 8% |
| Target-induced signal, 0.5 to 0.25 mm | 1.923% | 4% |
| Charge conservation including ground | 1.308e-12 relative | 1e-9 |
| Reciprocity | 2.206e-14 relative | 1e-10 |
| Reaction-charge / energy matrix agreement | 1.909e-12 relative | 1e-9 |
| Free-node residual | 1.185e-13 relative | 1e-9 |
| Larger grounded-domain target-signal change | 13.233% | 20% |
| Affine manufactured potential | 8.216e-15 absolute | 1e-9 |
| Layered dielectric analytic capacitance | 4.285e-14 relative | 1e-9 |

The layered analytic and numerical capacitances were respectively
26.5625634384000 and 26.5625634384011 pF/m. Background-equal target signal was
exactly zero and independent of position. Charge signs and independent
reaction/energy measurement consistency passed; the numerical tests additionally
checked PSD and the global-voltage gauge mode.

These gates validate the defined finite-domain prototype at the tested probes.
They do not establish an open-domain limit, full 3D realism, or numerical errors
below noise. At the strongest near-electrode probe, 1-to-0.5 mm signal discrepancy
was **14.47 times sigma in Euclidean channel norm** (not a covariance-weighted
Mahalanobis ratio). Far-field domain dependence is still appreciable. The
initial nonconforming Python development model failed mesh/domain gates and is
retained in [the failed report](../results/development-validation-python-failed.json);
no performance results came from it. [Amendments](protocol-amendments.md).

## Conventional localization: actual test results

The estimator jointly fits x,y and unknown relative permittivity in [2,8] by
noise-normalized bounded GLS with five separated multistarts. It interpolates
a **270-state** 1 mm FEM dictionary. Observations came from a refined 0.5 mm
mesh; most target states were continuous off-grid positions/materials. The
ambiguity scenario deliberately uses dictionary-selected difficult states on
the refined mesh. Empty histories are the only per-sequence calibration input.

Three seeds; 210 whole sequences; **2,520 observations** (720 training, 720
calibration, **1,080 test**). Ten scenarios, each 9 independent test drift/noise
sequences with 12 observations. All methods here mean the single conventional
estimator; acceptance is **100%** throughout. Position-error tolerance was 5 mm.
Fixed physical pools are shared as experimental design across independently
simulated sequences. Intervals resample complete sequences, conditional on that
design, rather than treating individual time frames as independent.

| Scenario | Mean mm [sequence-bootstrap 95% CI] | Median mm | p90 mm | Error >5 mm | Fits without convergence flag |
|---|---:|---:|---:|---:|---:|
| Clean | 0.817 [0.719, 0.919] | 0.671 | 1.525 | 0/108 (0%) | 27.8% |
| Mild drift | 0.837 [0.695, 0.973] | 0.580 | 1.896 | 0/108 (0%) | 30.6% |
| Strong drift | 1.360 [1.195, 1.552] | 0.913 | 3.010 | 2/108 (1.85%) | 38.0% |
| Wider dielectric | 0.791 [0.632, 1.034] | 0.462 | 1.758 | 2/108 (1.85%) | 48.1% |
| Combined mismatch | 1.407 [1.187, 1.636] | 0.721 | 3.254 | 6/108 (5.56%) | 38.9% |
| Poor sensitivity region | 2.192 [2.028, 2.359] | 2.120 | 3.659 | 1/108 (0.93%) | 50.9% |
| Near-ambiguity pool | 1.813 [1.685, 1.940] | 1.591 | 3.370 | 2/108 (1.85%) | 61.1% |
| Correlated disturbance | 0.946 [0.786, 1.153] | 0.739 | 1.769 | 0/108 (0%) | 25.9% |
| Channel disturbance | 1.335 [1.197, 1.487] | 0.681 | 3.916 | 6/108 (5.56%) | 36.1% |
| +10% radius mismatch | 0.627 [0.513, 0.747] | 0.442 | 1.189 | 1/108 (0.93%) | 60.2% |

Favorable and unfavorable findings both matter. Strong drift raised mean error
relative to clean in this pilot. Weak-sensitivity and near-ambiguous design pools
gave larger errors. Wider dielectric and radius mismatch did **not** simply
worsen average position error; their means were lower than clean, with overlapping
sequence intervals for wide material. That is an observation from these pools,
not evidence that mismatch improves sensing or that the fitted material is correct.

Noiseless off-grid inverse self-checks against the estimator's own interpolation
recovered position to <1.4e-11 mm and material to numerical precision at three
states. This tests optimization/interpolation consistency, not an independent
accuracy claim. Nine Rust numerical tests passed, including a separate synthetic
identifiable inverse benchmark and an off-grid refined-FEM mesh test.

The **nonconvergence flags are material**: 25.9–61.1% by scenario. A low-error
returned point does not make a fit globally optimal. Some projected steps/boundary
solutions or piecewise interpolation knots do not meet stopping criteria within
100 iterations; exact cause is not established for every flagged fit. The
always-return baseline preserves those outputs and their flags. It was not tuned
after final labels were inspected. A robust optimizer study must use fresh
development states and a newly frozen evaluation; these results are a pilot.

## Ambiguity and failure evidence

The smallest dictionary Mahalanobis separation was **1.935** for two states
(-18,28) mm and (-24,28) mm, both epsilon_r=2, separated by 6 mm. This finds a
noise-sensitive alternative, not exact equality or proof of global uniqueness.
The finite search allows different material values and excludes separations <=5
mm. A background-equal target is demonstrably unlocalizable everywhere.

The largest test error was **10.116 mm**, combined mismatch, sequence 5404729,
frame 11: truth (0.478,28.000) mm, epsilon_r=5.211; estimate (10.415,26.106) mm,
epsilon_r=2.111, normalized residual 11.684. The solver marked it converged.
Convergence therefore does not imply accurate localization. A near-ambiguity
failure had 7.086 mm error and residual 3.576; residual size alone need not
separate every large error. No rejection ranking was tested.

Runtime for the [uncached baseline run](../results/first-run-summary.json) was **222.0 s**, including dictionaries
and refined physical pools. Median/p90 point inference was **0.149/0.667 ms**;
process peak working set **78,716,928 bytes (75.1 MiB)** on the recorded Windows
host under x86_64 execution. These timings exclude the earlier compilation and
full validation run; the corrected cached rerun took **0.206 s** including output
generation. Plot previews
were rendered in headless Chrome and visually inspected.

## Artifacts and reproducibility

[Validation](../results/validation.json), [summary](../results/summary.json),
[test log](../results/numerical-tests.log), [CSV](../results/localization.csv),
[observations](../results/dataset.json), [predictions](../results/predictions.json),
[ambiguity](../results/ambiguity.json), [sensitivity](../results/sensitivity.json).

![Baseline scenario results](../results/plots/scenario-comparison.png)

[Clean error map](../results/plots/localization-error-clean.svg),
[strong-drift error map](../results/plots/localization-error-strong_drift.svg),
[wider-material error map](../results/plots/localization-error-wide_dielectric.svg),
[ambiguity map](../results/plots/ambiguity.svg),
[initial sensitivity map](../results/plots/sensitivity.svg).
Map cells show mean sampled error; missing cells are explicitly n/a. Diagnostic
maps use forward dictionary states, not inference-time true positions.

Configuration SHA-256:
`dfa96dab7da944361ba8b9fe39b6f81a60df8e3511cb1e1f53c39292dbad10a1`.
Dataset SHA-256:
`cf3a2b5d9c7beabbdf877bcb637552ee6ba7d9c27c7743049f7710439a821617`.
The dataset was saved before predictions, and predictions before test-label
evaluation. The configuration and dataset hashes are embedded in the summary.

The first cached replay exposed a JSON parser float-rounding issue. Enabling
exact float round trips corrected it without changing any scientific settings.
The 20,000-float regression test passes, and the corrected cached run reproduced
both the original dataset and localization CSV **byte for byte**. Runtime fields
are naturally variable. [Replay evidence](../results/reproducibility-check.json).
The failed replay summary is retained; the current results reproduce the initial
uncached scientific results exactly. The first run and corrected cached run have
separate runtime/memory records.

## Research decision at this milestone

The prototype supports further **bounded numerical and identifiability work**.
It does not yet justify an SSRN manuscript claiming a selective advantage,
electrode co-design, or hardware/3D expansion. The strong-drift pilot had only
two >5 mm errors in 108 observations, so a later selective comparison needs a
power/sample-size assessment before new final data are drawn. The declared
2-point practical risk difference may be difficult to resolve in this regime.
Do not change this pilot's drift or tolerance post hoc to manufacture an effect.

Before Milestone 3: establish a stronger optimization termination/global-search
reference and quantify interpolation mismatch using untouched development
states; then freeze the common information budget, priors, C2 expected-error
loss and D/ablation definitions. New complete test sequences are required since
the present results have been viewed. If C2 matches or beats D, report it and
consider stopping. [Full limitations](limitations.md),
[literature/novelty boundary](literature-review.md).
