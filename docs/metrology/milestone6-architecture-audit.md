# Milestone 6 architecture audit

Starting revision: ea15d40 (master fetched before changes, October 11, 2026).

The generic Rust evaluator has typed, strict prediction records, calibration-only quantile thresholds, tie-aware risk-coverage curves, event Brier/ECE, missing-label handling and exact replay. Its sequence bootstrap assumes independent groups; adjacent Electricity days do not justify that assumption. The new benchmark must provide explicit moving-block sensitivity instead. Existing numerical flags are attestations, not verified certificates. Preserve that distinction.

The conservative abstention controller uses simultaneous Hoeffding bounds for fixed candidate thresholds and independent equal-length sequences. Electricity's adjacent periods do not satisfy those assumptions. It remains unchanged and is not presented as providing a temporal guarantee. The benchmark's quantile threshold is a different, descriptive operating point.

Milestone 5 trained a synthetic logistic model and exposed severe shifted risk at similar coverage; it did not establish real-world utility or independent scientific replication. Numerical sensor gates remain failed. The benchmark extends the generic input/evaluator through a separate module rather than changing historical scientific calculations.

New contract: source provenance, strict ordered records, explicit windows and label-free diagnostic scores, calibration-frozen alerts, paired moving-block uncertainty, and replayable reports. Conventional, selective and gated outcomes use identical predictions. The simple combined-warning baseline exactly matches TRIAD's gate; any difference would be an implementation bug. Numerical validity is not applicable to these tabular models; statistical insufficiency and unresolved chronology are contextual cautions, not predictive detectors.

CI already covers Rust formatting/tests/lint, synthetic independent checks, NIST format and portable contracts on Linux and Windows. Extend it with benchmark replay, adversarial cases and independent NumPy checks. Publication is via a development branch and PR, with all checks green before merge. Repository protection visibility is checked separately; inability to inspect administrative settings is not permission to bypass CI.

Dataset feature-only audit: 45,312 rows, 944 consecutive 48-period blocks, uninterrupted weekday cycle, no missing tokens in released data, five backward jumps in normalized dates and only 933 unique date values. The ARFF relation declares normalization and ReplaceMissingValues. Do not sort by this date field or claim verified calendar chronology. Split the published ordered stream and explicitly qualify all temporal interpretations. Current NSW price defines the class and must not enter predictors. Prior-day measurements and known periodic calendar features are used; source-wide preprocessing is an irreducible limitation.

The protocol is written before fitting models or summarizing final labels. This is local exploratory prespecification, not independent registration. A separate hash manifest protects all previously tracked results and experiment files, including Milestone 5.
