# Milestone 4 development diagnostic: first numerical component results

**Executed October 11, 2026, on a separate research branch.** Status: exploratory development finding. **Numerical readiness FAILED.**

This report describes `triad numerical-components` and the immutable [machine-readable ten-probe record](../results/numerical-components-v4.json). It does **not** revise the [original Milestone 3 study](milestone3-results-2026-10-11.md), change the physics likelihood used in v3, or claim a new selective-inference improvement.

## Question

How much of the known discrepancy is attributable to interpolating a coarse 270-state dictionary, versus solving the physical FEM problem on a too-coarse mesh?

The original dictionary interpolation is evaluated against **exact same-state 1 mm FEM**. Mesh error is evaluated between direct 1 mm and 0.5 mm FEM solutions, again at exactly the same state. The total is the dictionary or candidate prediction relative to the direct 0.5 mm solution. All error magnitudes are full covariance-whitened channel norms, **not** position error in millimeters.

The local candidate divides each original x, y and dielectric interval into four subintervals, sampling eight FEM corners near each known diagnostic state (the four-by-four-by-four grid is *not* fully instantiated). Trilinear interpolation of those eight signals gives a locally refined forward approximation **for diagnosis only**.

## Recorded results

Ten deterministic development probes: six already published in v3, plus four additional development states. **They are not independent confirmatory test probes.**

| Maximum across all 10 probes | Original approach | Local-stencil candidate |
| --- | ---: | ---: |
| Forward interpolation error vs direct 1 mm FEM | 29.862 noise units | **2.276 noise units** |
| Total approximation error vs direct 0.5 mm FEM | 37.778 noise units | **10.482 noise units** |
| Mesh-only 1 mm vs 0.5 mm FEM disagreement | 9.821 noise units | **9.821 noise units** (unchanged) |

Maximum interpolation-only disagreement fell by about **92.4%** on these observed development probes. The total-model maximum fell by about **72.3%**. These max differences are aggregated across probes; the original and candidate maxima need not occur at the same state.

The near-electrode probe `[-19.3, 9.7] mm, epsilon_r = 5.4` had original dictionary-to-refined mismatch **37.778**, reduced to **10.395** by the local stencil. At this *same* probe the exact 1 mm-to-0.5 mm mesh disagreement was already **8.138**, which no 1 mm local interpolation scheme can remove.

The additional development probe `[-22.1, 9.1] mm, epsilon_r = 7.4` revealed a mesh discrepancy of **9.821**. On this probe, the local candidate has a total mismatch **10.482**. Both exceed the provisional one-noise-unit inspection threshold.

**The readiness flag is `false`.** This is not evidence of calibrated C2/D uncertainty, lower physical sensor error, improved geometry, or a validated 3D model.

## What this diagnoses and what it does not

- **A concrete engineering direction exists:** localized forward sampling can substantially reduce coarse interpolation error in the examined regions.
- **The remaining primary obstacle is mesh and modeling error,** especially near the electrodes. Refining interpolation alone cannot repair it.
- **The candidate is not deployable inference.** In the diagnostic, the true target state selects the local stencil. In a real estimator, target location and permittivity are precisely the unknowns. An operational adaptive dictionary would require a coarse estimate or global search to choose refinement cells using measurements only, account for multiple plausible regions, and measure its own computational cost.
- **No independent validation claim:** the reused v3 development probes and additional v4 development probes were deliberately selected for numerical debugging. Future gate results require newly frozen independent validation positions and physically credible noise treatment.
- **No retuning v3:** its failed gate and unsupported scientific hypothesis remain archived exactly as recorded.

## Next numerical milestones

1. Localize errors in space, material and electrode edge singularities; refine the target-conforming mesh and test direct 0.5 mm-to-0.25 mm convergence in covariance-whitened units for untouched numerical validation states.
2. Build inference-time local refinement driven **only by observed signals**, with a stable uncertainty envelope and a budget for all plausible target locations, not ground-truth probe locations.
3. Establish an independently evaluated numerical error budget below its justified tolerance across near-electrode, mid-field, boundary-sensitive and unusual-material cases.
4. Only then freeze the power assessment and new independent C2/D selector test described in the [Milestone 4 plan](milestone4-plan.md) and [planning-only configuration](../experiments/milestone4-design.json).

**Provenance:** GitHub Actions development run [38162814785](https://github.com/topherchris420/triad/actions/runs/38162814785); `cargo test --release --locked` passed, and the original Milestone 2 and Milestone 3 artifacts were checked as unchanged. Do not treat the build status as scientific success.
