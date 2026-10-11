# Milestone 5 executed results â€” October 11, 2026

**Engineering progress; numerical readiness failed; no confirmatory scientific
claim.** All experiments below were actually executed locally with Rust release
builds on Windows. Timing is descriptive wall time with concurrent workstation
activity, not a controlled performance benchmark.

## Numerical study

[Frozen protocol](../../experiments/metrology-numerical-v5.json),
[complete results](../../results/metrology-numerical-v5/summary.json).
Four new development states, two layouts, 1/0.5/0.25 mm meshes, expanded boundaries,
background drift, and an optional edge/interface-refined 0.5 mm candidate were run.
All eight probe/layout records and their full signal/solver diagnostics are saved.

| Maximum full covariance-whitened discrepancy | Noise units |
| --- | ---: |
| Original 1 mm versus 0.5 mm | 13.2783 |
| Original 0.5 mm versus 0.25 mm | 6.2147 |
| Edge/interface candidate versus original 0.25 mm | 1.1095 |
| Expanded domain sensitivity at 0.5 mm | 37.9810 |
| Background dielectric drift sensitivity | 14.3669 |

The candidate reduced disagreement at every sampled probe/layout, but its worst
case still exceeds the provisional one-noise-unit inspection limit. Baseline
three-resolution solver residuals passed 1e-9. Large boundary sensitivity is a
separate unresolved adequacy concern; it is not repaired by interpolation or
larger assumed sensor noise. The drift values describe sensitivity, not numerical
truncation error. No continuum error bound, global error distribution, 3D model,
or hardware validation follows. These new probes are now exposed development
evidence and must not be reused as untouched confirmatory validation.

## Measurement-driven refinement: negative development comparison

[Protocol and results](../../results/metrology-adaptive-v5/summary.json).
Two new seeded development observations selected two candidate cells using
measurements only. Both candidates performed 45 unique FEM solves with nine
within-call cache hits. All plausible sampled states/residuals remain in output.

| Case | Coarse continuous estimator error (mm) | Adaptive 1 mm grid (mm) | Bounded direct 0.5 mm grid (mm) |
| --- | ---: | ---: | ---: |
| 0 | 0.6241 | 0.9899 | 0.9899 |
| 1 | 1.3737 | 1.9235 | 1.9235 |

The adaptive and direct refined candidates **lost to the coarse estimator** on
both cases. Adaptive calls took about 9.44 and 9.09 seconds; direct refined calls
took 34.38 and 48.50 seconds, versus roughly 2.04 and 0.32 milliseconds for the
coarse estimator. Grid quantization and the bounded shortlist can lose accuracy;
these are not exhaustive continuous refined inversions. Both candidates abstain
from validated physical confidence. No superiority claim is supported.

## Trained non-capacitive AI demonstration

[Model](../../results/metrology-demo-v5-canonical/model.json),
[predictions](../../results/metrology-demo-v5-canonical/predictions.json),
[machine report](../../results/metrology-demo-v5-canonical/report.json),
[readable report](../../results/metrology-demo-v5-canonical/report.md).
Logistic regression was actually trained on 800 independent synthetic samples.
Development, calibration and two evaluation streams use separate fixed seeds.
The calibration-only threshold is 0.3549937151721641 for nominal 80% coverage.

| Evaluation | Coverage | Accepted failures / accepted | Selective risk | ECE | Brier |
| --- | ---: | ---: | ---: | ---: | ---: |
| In distribution | 82.3333% | 86 / 494 | 17.4089% | 0.04243 | 0.15259 |
| Concept shift | 80.1667% | 405 / 481 | 84.1996% | 0.58035 | 0.57163 |

The shift reverses feature effects without refitting. Similar coverage conceals
dramatically worse risk. This establishes that the evaluator calculates metrics
for a trained non-capacitive model, **not** that physical likelihood validation
generalizes to all AI or that the selector is shift-robust.

## Planning and verification

[Planning output](../../results/metrology-planning-v5.json) includes a separately
seeded development pilot, 36 analytic sensitivity settings, and 24 paired nested-
pool simulation settings with 200 replicates each. Rates, effects, dependence and
pairing are assumptions; the synthetic pilot is not a physical C2/D pilot. Final
sample sizes and practical utility are not justified, and a confirmatory protocol
was not frozen or run while numerical readiness fails.

51 Rust tests passed, including analytic physical checks, historical artifact
hashes, split separation, exact metrics, ties, bootstrap, adversarial failures,
CLI overwrite protection, and refinement stability. Formatting and compilation
passed. Clippy completed with four pre-existing style warnings; transitive `nom`
and `quick-xml` have future-compatibility warnings. The independent Python checker
recomputed coverage, risk, incorrect acceptance, AURC, ECE and Brier successfully.
[Replay checks](../../results/metrology-reproducibility-v5.json) confirmed identical
trained model, predictions and deterministic report payloads; timing is excluded.
All **63** baseline-hashed historical files remained byte-identical.

NIST schema validation failed only the intentionally absent contact field. The
package is a draft; scientific distinctiveness, replication, contact and human
review remain unmet. Nothing was submitted to NIST.

Next independent experiment: freeze new near-edge probes, independently vary
edge-node density and interface polygon resolution, and repeat the expanded-domain
comparison with matched discretization. Establish a defensible numerical error
budget before collecting an independent physical C2/D pilot or final test set.

Portability note: the initial `metrology-demo-v5` snapshot preserves the first
Windows run with CRLF source bytes. The canonical snapshot uses Git LF source
bytes and is the published reproduction target; scientific metrics and predictions
are identical. The evaluator source hash correctly distinguishes these byte
representations. Use the canonical report with code checked out from Git.
