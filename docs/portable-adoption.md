# Portable abstention and sensor capture: engineering adoption gate

**Status: implementation and unit-test work, NOT a validated scientific or hardware result.**
TRIAD's published Milestone 3 remains null/negative, and Milestone 4's 1 mm-to-0.5 mm FEM noise-normalized discrepancy still fails its numerical readiness inspection. These utilities are a reusable boundary around that evidence, not a way to reinterpret it.

## A. Conservative abstention on independent sequences

The public Rust library exposes triad::abstention::{fit, FitSpecification, CalibrationSequence, FittedPolicy, Decision}. It is deliberately independent of FEM, electrode geometry and the TRIAD estimator. Existing method C2 or D can supply a risk score, but this controller does not make either scientifically valid or calibrated by itself.

CLI, JSON input and JSON output:

    cargo run --locked -- calibrate-abstention --input examples/abstention-calibration-demo.json

**Expected outcome for the demo: refused.** Four illustrative calibration sequences do not establish enough statistical precision to certify even the illustrative target. There is no real device data or genuine estimator result in this fixture.

The input contains a predeclared method identifier, a maximum *conditional* error rate, a minimum acceptance coverage, a family-wise delta, and a finite fixed list of candidate thresholds. Each independent sequence has an ID and the same number of frames. A frame carries (1) a finite score in which *smaller is safer*, and (2) a verified Boolean bad-event label. A future study must declare the millimeter error threshold before measuring those labels. The user must preserve sequence independence, not split one drift session across calibration and testing, and not derive candidate thresholds from the same calibration measurements.

For each fixed threshold t, let A be the fraction of accepted frames and B the fraction of accepted incorrect frames *within a sequence*. Their empirical means over n independent sequences are a and b. For K predetermined candidates and family-wise failure probability delta, use the two-sided-family one-sided Hoeffding radius:

    epsilon = sqrt(log(2 K / delta) / (2 n))
    lower_coverage(t) = max(0, a - epsilon)
    upper_joint_error(t) = min(1, b + epsilon)
    upper_conditional_risk(t) = min(1, upper_joint_error / lower_coverage)

If lower_coverage is zero, the risk upper bound is 1. This construction takes a union bound over the 2K one-sided inequalities for a **fixed** candidate set, without assuming independent frames within a session. In this design, all sequences have the same frame count so the ratio targets a random sequence and frame. A ready policy requires *both* the coverage lower bound and the conditional-risk upper bound to meet their declared limits. If no candidate passes, the policy is refused, even if the empirical error rate is zero.

**Sharper certification.** The same input can be certified with
`certify-abstention`, which tests the identical family-wise claim with a linearised
risk and Hoeffding–Bentkus bounds. It provably qualifies every threshold this
controller qualifies, and needed about a tenth of the independent sessions in the
pre-registered Milestone 9 simulation. `plan-certification` estimates the number
of sessions to collect. See [certification](certification.md).

A fitted policy's decide(Some(score)) returns accept only when the policy is ready, its selected threshold corresponds to a qualifying candidate, and the finite score is no greater than the threshold. Missing/nonfinite scores, invalid thresholds, and uncalibrated policies withhold the prediction. Consumers must also bind the policy to exactly the correct model, preprocessing, units and population; method_id is descriptive metadata, not a cryptographic lock.

**Statistical assumptions and caveats:** The sequences must be independent, identically distributed samples from the intended deployment population. Calibration ground truth must be reliable, candidate thresholds precommitted, scores computed without the ground-truth label, and estimator/scoring code frozen before calibration. An adversarial environment, device-to-device variation, site shift, non-stationary temporal drift, incorrectly estimated FEM fields, or variable sequence selection can invalidate the nominal bound. No universal coverage or safety guarantee is claimed.

### Minimal library consumption

    use triad::abstention::{Decision, FittedPolicy};
    fn should_display(policy: &FittedPolicy, score: Option<f64>) -> bool {
        policy.decide(score) == Decision::Accept
    }

Do not report a percentage confidence when the policy refuses. The result is a decision, not a replacement for original measurements.

## B. Physical capture interchange and noise inspection

Run:

    cargo run --locked -- inspect-capture --input examples/synthetic-capture.json

The committed capture fixture is **synthetic** and demonstrates only file parsing and descriptive statistics. To ingest actual hardware, build a version-1 JSON capture with:

- capture_id, device_class and geometry_id identifying a reproducible session/configuration, preferably without personal data or secrets;
- space = physical3d and unit = pf, with absolute readings rather than a 2D pF/m-derived value;
- channel_pairs = [[0,0],[0,1],[0,2],[0,3],[1,1],[1,2],[1,3],[2,2],[2,3],[3,3]] following the forward model's **explicit channel/sign convention**;
- strictly increasing timestamps in milliseconds, plus at least three empty_reference frames and optional target_present frames;
- each frame's channels array containing exactly ten finite readings with consistent excitation and electrode order.

The physical instrument must establish whether self/mutual capacitance and readout signs actually match this mapping. An incompatible readout needs an explicit, documented electrical calibration/adapter, not guessed sign flips or a silent channel permutation.

The audit returns per-channel empty-reference means, sample standard deviations, the sample covariance and late-minus-early reference means. These are **descriptive baseline-variation diagnostics**. They conflate electronic noise, drift, temperature, parasitics and environmental changes, and short sample covariance matrices can be singular. They do not estimate target positions, validate real sensor noise assumptions, or produce calibrated uncertainty.

**Non-negotiable units boundary:** physical three-dimensional capacitances are in pF. The current 2D cross-sectional solver outputs capacitance **per omitted length** in pF/m. Direct insertion of pF data into its 2D likelihood is dimensionally wrong. The importer refuses mismatched physical-space/unit pairs and performs **no** automatic dimensional conversion or inverse inference.

### Next physical experiment (not implemented)

1. Obtain a repeatable, documented four-electrode PCB/sensor geometry with grounded enclosure, substrate and shielding data. Record actual electrode readout order, frequency, voltage, sampling period, calibration mode and controlled target configuration. Capture serial/device settings in a separate, privacy-safe manifest.
2. Record unoccupied/baseline sequences on multiple runs, times, temperatures and devices. Estimate drift and covariance on development runs only; test held-out devices and sessions without retraining. Compare real covariance with the synthetic SIGMA/RHO assumptions.
3. Implement and validate a **3D electrostatic solver** for finite electrodes, substrate, parasitics, dielectric shape and boundary conditions. Compare against independent analytical limits, mesh refinement and preferably a separate FEM backend. Require numerical mesh errors small relative to *empirically measured* noise.
4. Calibrate a physically meaningful 3D forward model and inverse estimator from development data. Evaluate position on externally measured ground truth, including nonconvergence, field ambiguity, uncertainty calibration, and selective risk under genuine environmental shift.
5. Pre-register the geometry and C2-vs-D comparison, power analysis, fixed thresholds and independent final sessions before collecting or exposing final labels. Report null outcomes and original baseline results unchanged.

## C. Scientific integrity: what this does and does not fix

| Limitation | Added capability | Still missing |
| --- | --- | --- |
| Unsupported selective hypothesis / numerical gate failure | Independent-session conservative risk/coverage gating usable outside TRIAD; no forced acceptance of an unready policy | Passing 2D mesh/noise error budget, deployable inference refinement, powered fresh confirmatory C2-vs-D trial |
| Synthetic-only 2D physics/no actual noise | Strict capture interchange, dimensional distinction and descriptive real-data diagnostic path | Real captures, 3D FEM, electrical calibration, hardware ground truth and device/site holdouts |
| Narrow software impact | Exportable Rust types, JSON CLI, stable example contracts and regression tests | Independent downstream integrations and demonstrated external performance |

**No committed fixture is real data. No new selective experiment was run, and no numerical or scientific gate was changed to make it pass.** Existing v3/v4 evidence must remain intact.