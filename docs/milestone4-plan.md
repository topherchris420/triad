# Milestone 4: Numerical reliability before renewed selective inference

**Status: the first numerical development diagnostic has been executed; a renewed selective scientific test is NOT yet authorized or preregistered.** Read the [numerical results](milestone4-development-results-2026-10-11.md). They show substantial local interpolation improvement but a failed noise-normalized mesh readiness gate.

This plan follows the [executed and archived v3 experiment](milestone3-results-2026-10-11.md). No v3 observations, fitted predictions, experimental outputs, protocol constants or evaluation labels are overwritten. The numerical v3 gate failed; physics-informed D showed only a small unvalidated ranking advantage.

## Workstream A: isolate the numerical error budget

The new `numerical-components` command has created and archived `results/numerical-components-v4.json` using ten declared **development-only** probes. Six were already examined in v3 and four are additional engineering probes. These cannot serve as untouched final confirmation.

For each state, compare three independent quantities under the **full covariance-whitened metric** used by the inference model:

1. **FEM mesh discrepancy:** the direct 1 mm model versus a direct 0.5 mm model at the *same* target state and material. This measures mesh-dependent disagreement, not inference error.
2. **Dictionary interpolation discrepancy:** the original trilinear 270-state dictionary prediction versus the direct 1 mm model at the same state. This isolates surrogate interpolation from cross-mesh disagreement.
3. **Total forward-model discrepancy:** the dictionary signal versus the direct 0.5 mm model. This measures their joint effect (which need not equal the sum of the norms because errors can cancel).

The command also tests **an exploratory local stencil**: for each diagnostic state, split its three original dictionary intervals into four subcells, compute the eight local trilinear corners with new 1 mm FEM solves, and measure local interpolation against both direct 1 mm and 0.5 mm signals.

This local-patch experiment is *not* installed as the inference dictionary, posterior, or estimator; it is not a new sensor result. If it improves interpolation, a future algorithm must make local sampling computationally feasible **without using hidden target truth** to choose the patch. The diagnostic knows the true probe because it is studying forward approximation, not predicting measurements. You cannot count that as deployed estimator performance.

The whitened distances must obey the triangle inequality. Tests check exact interpolation of synthetic trilinear functions, rejection of extrapolation, and the archival integrity of v3 outputs.

**Crucial:** even zero interpolation error would not repair excessive mesh error. Future studies must separately investigate electrode singularities, interface discretization and graded mesh refinement, boundary sensitivity, and solver convergence. The old relative mesh validation gates were not noise-normalized and therefore could pass despite material likelihood mismatch.

## Workstream B: statistically feasible practical-effect definition

The frozen v3 study required a **2-percentage-point absolute reduction** in accepted-error risk at 80% coverage. The observed C2 accepted-error risk was only `42 / 2880 = 1.4583%`, so a 2-point improvement was unattainable for *that exact test set*. This is retrospective information, **not** a claim of mathematical impossibility for every possible future population.

The new [planning-only design](../experiments/milestone4-design.json) illustrates a feasible, *provisional* target: **at least 0.2 percentage points absolute risk reduction** and **at least 15% relative reduction**, while maintaining matched 80% accepted coverage. Those illustrative figures are chosen after reviewing v3 and thus do not convert any v3 evaluation into a confirmatory result. They must be justified on scientific significance and **powered** via simulated sequence/site-level sampling before a distinct future protocol is frozen.

Code in `study_design::risk_reduction_is_feasible` guards against asking for a risk reduction larger than observed baseline risk on a fixed evaluation dataset; this is an arithmetic sanity check, **not a power analysis**.

## Workstream C: what must happen before new test sequences exist

1. Improve and independently validate the FEM mesh and 3D-extruded conceptual distinction. Establish a noise-normalized error budget across near-electrode and far-field regions, target materials and layouts, with independent probes not selected using old results.
2. Establish a fitted inverse model that uses the improved forward approximation **at inference time**; freeze the observation/calibration budget, approximate C2 posterior definition, D physics-weight selection and optimizer stopping diagnostics using development data only.
3. Simulate sampling variability at the **whole-sequence and spatial-pool** levels, not just frame level; choose sample counts with enough >5 mm events to resolve the *preregistered* absolute and relative effect thresholds.
4. Freeze a new code commit, `v4` versioned configuration, development/calibration/test seeds and held-out positions **before any new final test evaluation**. Require no retrospective tuning against exposed v3 test labels.
5. Run an independently seeded selective test. Report actual deployment-threshold coverage separately from exactly matched retrospective test-score ranking, with intervals and null/adverse outcomes. If C2 wins or numerical readiness fails, explicitly report that.
6. Revisit electrode geometry only with its own calibrated inverse model and independently generated holdout, not with pairwise separation alone.

**Research boundary:** the current code does not establish v4 numerical readiness or the v4 selective hypothesis. Keep v3 as an accessible negative/inconclusive study and do not cite its result as evidence of commercial or physical superiority.
