# Milestone 3: First completed experiment, October 11, 2026

**Status: executed and archived; scientific hypothesis NOT supported.** These are exploratory synthetic 2D simulation results, not physical sensor validation. The original Milestone 2 baseline and its configuration remain unchanged.

- Source revision: [`da8c38c`](https://github.com/topherchris420/triad/commit/da8c38cef256958a7bb77c705dda9e232fe44f168c4).
- [Successful GitHub Actions run](https://github.com/topherchris420/triad/actions/runs/38154107523) (Rust tests and all three research commands).
- [Original workflow artifact](https://github.com/topherchris420/triad/actions/runs/38154107523/artifacts/11707742513), with expiration under GitHub's retention policy.
- Permanent committed evidence: [numerical audit](../results/numerical-audit-v3.json), [selective summary](../results/selective-v3/summary.json), [test scores](../results/selective-v3/test-scores.json), [labeled evaluation](../results/selective-v3/evaluation.json), [run identity](../results/selective-v3/run.json), [geometry screen](../results/geometry-screen-v1.json). The selective summary records SHA-256 digests of its constituent files.
- The archived [baseline hashes](../milestone3-logs/baseline-before.sha256) were checked after the experiment; all listed frozen files passed.

## 1. Numerical readiness: failed

The independent six-probe numerical audit compared direct 1 mm and 0.5 mm finite-element signals, and compared the interpolated coarse inference dictionary against the refined physical model, using the declared *correlated-noise-whitened* metric.

| Numerical diagnostic | Maximum | Provisional inspection limit | Outcome |
| --- | ---: | ---: | --- |
| 1 mm to 0.5 mm FEM mesh discrepancy | 8.138 noise units | 1 | Failed |
| Interpolated dictionary versus refined FEM | 37.778 noise units | 1 | Failed |
| Optimizer objective excess above best dictionary candidate | 0.000 | 0.000001 | Passed at sampled probes |

The worst discrepancy occurs at development state `(x,y,epsilon_r) = (-0.0193 m, 0.0097 m, 5.4)`, near the electrode plane: 8.138 whitened units of mesh discrepancy and 37.778 units for the entire surrogate-to-refined-model difference. Other probes show varying discrepancies, so these six points do not establish a global bound.

**Implication:** The approximated likelihood in the newly implemented selective predictor cannot yet be trusted as numerically calibrated. In particular, the sampled optimizer check does not compensate for model discrepancy. The audit reports `likelihood_ready_for_claims: false`.

## 2. Selective prediction: small ranking advantage, below the declared bar

The versioned [protocol](../experiments/selective-v3.json) held the error tolerance at 5 mm, retrospective test acceptance coverage at 80%, and practically relevant difference at **2 percentage points**. Both methods had the same measurements, inference model, and point estimator. Fresh development/calibration/test streams were used for this v3 study. Test results should not be used to tune v3 and then called untouched.

There were 300 independent strong-drift test sequences, 12 frames each: **3,600 test predictions**, with **132 errors exceeding 5 mm before rejection**.

| Retrospective 80%-coverage metric | Posterior C2 | Physics-informed D |
| --- | ---: | ---: |
| Accepted predictions | 2,880 | 2,880 |
| Incorrect accepted predictions (>5 mm) | 42 | 37 |
| Conditional incorrect-accepted risk | 1.4583% | 1.2847% |
| Mean accepted error | 1.3202 mm | 1.3136 mm |

D's observed C2-minus-D risk reduction was **0.1736 percentage points**. The paired complete-sequence bootstrap 95% interval was approximately **[0.0347, 0.3819] percentage points**. This is an exploratory ranking result; it is far below the predeclared 2-point practical threshold, and **the numerical readiness gate failed**.

The separately calibrated *operational* score thresholds were selected exclusively from calibration sequences. On test, observed coverage was 77.75% for C2 and 77.08% for D; accepted error risks were 1.2147% and 1.0450%, respectively. These operational outcomes are at **different test coverages**, so do not present their difference as the equal-coverage primary comparison.

The report correctly records `supports_declared_primary_hypothesis: false`. A nominally positive bootstrap interval cannot override the predeclared practical threshold or invalid numerical likelihood assumptions. Likewise, this simulation does not establish hardware performance.

## 3. Electrode geometry: development winner failed to beat baseline on holdout

Five equal-width four-electrode layouts were screened at the same channel count and assumed noise. The screen selected **split_outer**, electrode centers `[-30, -8, +8, +30]` mm, on development data. The preexisting baseline centers are `[-27, -9, +9, +27]` mm.

| 10th-percentile whitened pairwise separation | Baseline | Selected split_outer |
| --- | ---: | ---: |
| Development sites | 23.312 | 23.410 |
| Independent holdout sites | **17.990** | 17.202 |

Higher is better for this *specific surrogate*. The selected geometry's holdout score was about **4.4% below** baseline. The 24-site holdout and pairwise-distance objective do not prove the baseline is optimal for localization or selective risk. No physical-device geometry advantage is established.

## Research decision and next experiment

**The experiments executed successfully. The v3 primary scientific claim did not pass.** Preserve these outcomes as they stand.

1. Isolate the coarse dictionary interpolation component from the FEM discretization component at the *same* physical states and mesh; audit worst regions, particularly near the electrodes. Test higher spatial/material dictionary resolution and targeted mesh refinement on **development probes**, not the recorded test labels.
2. Recheck the physically modeled field, boundary sensitivity, and full covariance-normalized discrepancy after numerical changes. Develop a defensible model-discrepancy uncertainty treatment rather than hiding mismatch by arbitrarily increasing the assumed sensor noise.
3. Freeze a **new versioned protocol, seeds, and independent test sequences** before repeating the selective comparison. Retain C2 as the primary comparator and publish null/adverse results.
4. Treat geometry co-design separately, with its own calibrated inverse model, numerical gates, and independent holdout. Do not market `split_outer` as an improved sensor configuration.

These records represent one executed study and a research boundary, not independent scientific replication. See [research integrity](research-integrity.md) and [Milestone 3 protocol](selective-v3.md).
