# TRIAD next experiment: from inverse fitting to justified abstention

**State:** proposed investigation only. The frozen Milestone 2 data have already been analyzed. They are **development evidence**, not fresh final test evidence for Method D.

## Gate A: trustworthy optimization and numerical likelihood

Use newly generated development states, without reading future final labels, to evaluate:
- The 100-iteration projected damped Gauss–Newton termination path, including active box constraints and interpolation knots.
- The best discrete grid candidate and a stronger independent search as optimization-quality references; report both objective gaps and stopping flags.
- Mesh-refinement errors in target-induced measurements using the *whitened Mahalanobis metric*, not solely Euclidean signal norm divided by sigma.
- The error between the continuous refined-FEM signal and the coarse dictionary's trilinear interpolation at the *same* target state.
- Whether numerical/interpolation errors dominate the nominal 0.003 pF/m channel noise in important regions.

This gate should clarify whether large errors are primarily caused by missing physical information, surrogate approximation, numerical PDE error, or inverse optimization.

**Stop condition:** If numerical or optimization artifacts dominate the apparent ambiguity, repair and revalidate on new development data before claiming a physical observability result.

## Gate B: define a shared inference information budget

All methods receive the same observed channels, initial empty-target calibration history, measurement geometry, historical observations allowed by the protocol, and training-derived nuisance priors. None receives true target material, position, drift, scenario identity or test-label metadata.

Define a proper likelihood that accounts for calibrated noise covariance and plausible but uncertain nuisance drift. Distinguish identifiable nuisance parameters from arbitrary offsets, which make single-frame localization unidentifiable.

Use a tractable grid or approximation that **retains multiple competing position/material explanations**. Verify posterior normalization, likelihood behavior, and numerical stability. Avoid premature claims of frequentist coverage under arbitrary environmental shift.

## Gate C: compare rejection criteria fairly

Use a common position estimator and probabilistic model as far as possible:

- **B:** noise-normalized residual.
- **C1:** posterior spread or variance, with multimodality retained.
- **C2:** posterior expected tolerance-exceedance loss for the reported point.
- **D0:** a physical observability score only.
- **D1:** posterior uncertainty plus nuisance-adjusted local observability.
- **D2:** D1 plus independent global ambiguity information.

A correct posterior C2 is Bayes-optimal for its own specified loss when correctly specified. If the physical score helps, clarify whether it does so through *robustness to misspecification*, a better approximation, or a previously omitted piece of information, rather than claiming that adding physics magically beats Bayes decision theory.

Never evaluate observability at the hidden true state. Use inferred states or posterior integration. Account for dielectric and constrained drift nuisance parameters in the sensitivity model.

## Gate D: pre-register a new, adequately powered test

The frozen primary question remains *incorrect accepted localization at 80% coverage*, under strong unseen drift, comparing D against C2, with a 5 mm error tolerance and a 2-percentage-point practical difference of interest.

**Pilot limitation:** strong drift produced only **2 errors >5 mm in 108 observations**. Rejecting up to 20% of samples gives little power to distinguish low risks with the current test size.

Before sampling:
1. Use independent **development** simulations for a power or precision study; determine the count of independent drift sequences needed for meaningful bounds on a 2-percentage-point difference.
2. Freeze scenario severity, target distribution, nuisance regimes, loss definition, predicted-position rule, confidence intervals, number of seeds and sample size before creating new final test sequences.
3. Calibrate operational acceptance thresholds using calibration sequences, and separately evaluate descriptive matched-80%-test-coverage ranking without turning that into a deployment claim.
4. Bootstrap or otherwise resample **whole independent sequences**, never frames as if independent.
5. Publish null, adverse, or inconclusive results. If C2 is as good as D, report it.

Do **not** quietly change the original 5 mm tolerance, enlarge drift severity after inspecting final predictions, or reuse the known test set to calibrate future methods.

## Gate E: evidence before expansion

Only after the complete C2-versus-D result is independently reproduced should TRIAD consider finite 3D electrostatics, a physical electrode board, electrode-geometry co-design, or an SSRN paper asserting a new method.

Source inspiration from [Infineon CAPSENSE](https://github.com/Infineon/capsense) and binary investigation via [REA](https://github.com/morluto/rea) remain permission-bound and separate from TRIAD's original implementation. No commercial sensor comparisons have been performed.
