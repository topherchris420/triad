# NIST AI Metrology Center: submission-readiness prototype

**Status: mechanically checkable prototype draft, not a NIST submission and not an independently validated methodology.**

Milestone 6 now supplies an executed external-data benchmark with independent
metric calculations. **It did not demonstrate added warning utility.** Combined
TRIAD warnings added no true degraded-window detections over covariate-only
warnings and increased false alarms. An identical simple warning gate produces
identical decisions. Read the [full real-world results](milestone6-real-world-results.md).
Distinctive scientific utility and unrelated-team replication remain unresolved.

Prepared October 11, 2026. The proposed contribution is **validity-qualified selective error risk at calibrated coverage**, not the invention of selective prediction or proof of physics-informed superiority. The correct comparison is established risk-coverage evaluation plus clearly separated applicability and numerical-evidence reporting. Reviewers should decide whether that integration is useful and sufficiently distinct.

## Official requirements and reproducible format check

- [NIST Metrology Center](https://airc.nist.gov/metrology/)
- [Submitting methodology](https://airc.nist.gov/metrology/how-to-submit/)
- [What reviewers look for](https://airc.nist.gov/metrology/how-to-submit/what-reviewers-look-for/)
- [Source-of-truth submission format](https://github.com/usnistgov/ai-metrology-submissions/blob/main/SUBMISSION_FORMAT.md)
- [Official v1 schema](https://github.com/usnistgov/ai-metrology-submissions/blob/main/validation/schemas/v1.json)

The [draft YAML](nist-submission-draft.yml) contains all seven required fields, including the user-provided public contact `ciao_chris@proton.me`. **The address becomes public if this draft is submitted; do not assume privacy.** Its scientific statements remain qualified.

`nist-schema-v1.json` is a vendored exact snapshot of NIST's official version 1.0 schema. Run from the TRIAD repository root:

```sh
python -m pip install "PyYAML==6.0.2" "jsonschema==4.23.0"
python scripts/validate-nist-draft.py
python scripts/validate-nist-draft.py --self-test
```

These commands are run in CI on Linux and Windows. They validate **format and required fields only**. Before any real external submission, re-fetch NIST's current schema, compare with the vendored copy, run NIST's own `check-jsonschema` and `validate_submission.py inspect` against the candidate located under a *local copy* of its `submissions/` directory, and manually review the citations and all URLs.

The [historical integrity manifest](baseline-hashes.json) protects 62 unchanged experimental and historical files. A later-edited living limitations document is explicitly excluded from the immutable test, with both hashes recorded; it has not been silently rebaselined. The original numerical evidence remains frozen.

The [schema validation record](nist-schema-validation.json) indicates what has actually been checked; do not confuse a local schema check with reviewer acceptance.

## Mechanical readiness

- [x] Single described measurement procedure with precise loss, coverage and abstention definitions.
- [x] All seven NIST mandatory fields included; professional contact provided.
- [x] Versioned schema and repeatable automated local-format validator included.
- [x] Relevant published selective prediction and calibration references identified.
- [x] Merged repository contains a working Rust evaluator and demonstration fixtures.
- [ ] Clean CI on the current master commit and separately confirmed links.
- [x] Fresh official-schema and local NIST native validation for the M6 draft (repeat before any future submission).

## Scientific and review readiness

- [x] The published example shows shifted predictions can be much less reliable at similar coverage; it does **not** show TRIAD prevents that collapse.
- [x] Known failure modes include false confidence under shift, dependence, missing truth and numerical inadequacy.
- [x] Negative Milestone 3 and 4 physics outcomes remain intact.
- [ ] Independent analysis of overlap with the current NIST catalog, and a concrete argument for added value relative to existing risk-coverage tools.
- [x] External real-world OpenML Electricity benchmark with two frozen models and independent NumPy computational checks.
- [ ] Reproduction and scientific assessment by a separate researcher.
- [ ] Scientifically justified use case and interpretation; uncertainty assumptions checked with the intended sampling population.
- [ ] Numerical adequacy and physical validation if claims about sensor-based confidence are pursued.
- [ ] Prospective adequately powered C2-versus-D confirmatory study **if** claiming superiority; no such study is required merely to describe a standard metric, but no physical superiority claim may be made without it.
- [ ] Human scientific review of scope, references, code behavior, and public disclosure.

## Honest submission positioning

Submit **one measurement method**, not an umbrella claim about trustworthy AI. The draft concerns evaluation of fixed predictors using labeled scores. It can report limitations of sensor-derived predictions, but it does not measure numerical adequacy automatically or certify devices. The NIST schema is a mechanical check; independent replication, distinctiveness and scientific quality remain open.

**No submission to NIST has been made.** Catalog inclusion, if it occurs in the future, does not imply endorsement, validation or fitness for a particular application.

## October 11, 2026 requirements and overlap review

The authoritative submissions repository was freshly fetched at
`7f0609217621aa905973ad0cc03f5e6a8c97e345`. Its v1 schema remains semantically and
LF-normalized byte-identical to the vendored schema (SHA-256
`f815015db4489f4c1a5de5d8f3b5a9913eeef1a8e65d2f36e8614be2ccd2da7c`).
Schema validation and the official local inspection procedure are recorded in
[the M6 format-check record](nist-schema-validation-v6.json). These checks only
evaluate submission format, not scientific acceptance.

The current [reviewer standards](https://airc.nist.gov/metrology/how-to-submit/what-reviewers-look-for/)
require metric-specific references, a usable merged implementation, accurate
metadata, practical interpretation and value beyond the catalog. Human review of
the evidence remains essential. The [catalog](https://airc.nist.gov/metrology/)
already includes Refusal Rate (fraction of refused prompts) and Test Fairness
(Well-Calibration, group-conditional positive-label probabilities), alongside
standard accuracy methods. These overlap in abstention/calibration concepts but
have different operational scopes. Established selective-prediction literature
already defines risk-coverage; TRIAD does not claim its invention. The candidate
contribution is an auditable evidence qualification workflow. M6 supports its
implementation and disclosure function, **not added predictive utility or
catalog-level novelty**. This is an internal comparison, not an independent
expert novelty review.

The public contact remains `ciao_chris@proton.me`. All changes remain a local
repository draft. No NIST-facing pull request or other submission is authorized
by the project's ordinary GitHub publication. A separate human scientific review
and explicit submission decision are required.
