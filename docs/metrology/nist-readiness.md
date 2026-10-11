# NIST AI Metrology Center: readiness assessment (Milestone 7)

**Recommendation: further scientific validation required.** Nothing has been
submitted to NIST. A separate human scientific review and an explicit submission
decision are still required. Catalog inclusion, if it ever occurs, does not imply
NIST endorsement.

Assessed October 11, 2026 against the official sources:
[AI Metrology Center](https://airc.nist.gov/metrology/),
[how to submit](https://airc.nist.gov/metrology/how-to-submit/),
[what reviewers look for](https://airc.nist.gov/metrology/how-to-submit/what-reviewers-look-for/),
and the [submissions repository](https://github.com/usnistgov/ai-metrology-submissions)
(`SUBMISSION_FORMAT.md` and `CONTENT_STANDARDS.md` at commit `7f06092`, schema v1.0,
byte-identical to the vendored [`nist-schema-v1.json`](nist-schema-v1.json)).

## What TRIAD measures, precisely

1. **Selective excessive-error risk at calibration-fixed coverage**: the fraction of
   accepted predictions whose loss exceeds a declared tolerance, with the
   acceptance threshold fixed on calibration scores. It also reports coverage,
   retrospective risk–coverage curves, event Brier/ECE and, for regression,
   normalised conformal coverage. These are **established** quantities
   (El-Yaniv & Wiener 2010; Geifman & El-Yaniv 2017; Brier 1950; Lei et al. 2018).
   TRIAD contributes an implementation, not the metric.
2. **Claim-relative evidence classification** (Milestone 7): a categorical
   assessment of whether typed evidence supports a typed reliability claim. This
   is a methodology, not a numeric metric. It is TRIAD's only candidate for a
   distinctive contribution.

## What Milestone 7 showed

- **Predictive utility: none.** On UCI Bike Sharing, TRIAD's evidence-qualified
  day decisions cost slightly more than standard monitoring at a matched
  calibration budget for both models (Δ = +0.0049 and +0.0062 per hour; below the
  practical threshold). This repeats the Milestone 6 Electricity finding.
- **Audit utility: shown only on an author-constructed suite.** 43/47 versus 20/47
  unsupported claims detected, with no false rejections. The rules, cases and
  ground truth share one author, and all misleading inputs defeated it.
- **Engineering: strong.** Frozen protocols, deterministic replay, an independent
  NumPy/SciPy verifier (3,027 checks), retraining checks, hash-protected history,
  Linux and Windows CI.

## Three stages of readiness

| Stage | Status | Basis |
|---|---|---|
| 1. Engineering ready | **Met** once the Milestone 7 pull request is merged with green CI | Working Rust implementation, 76 tests, deterministic replay, independent verification, reproduction package, schema-valid draft |
| 2. Scientifically reviewable | **Partly met** | The measurements and their limits are characterised; protocols were frozen before results; negative findings are reported in full. The evidence-sufficiency validation is circular (single-author suite), and no unrelated investigator has reproduced anything |
| 3. Compelling catalog candidate | **Not met** | No peer-reviewed reference about the evidence-sufficiency method itself (reviewers require 1–2 references about the metric); no demonstrated utility beyond existing methods; distinctness from assurance-case practice and checklists such as REFORMS not established |

Passing the schema shows only that the YAML is well formed. It is **not** readiness.

## Content standards self-check (official `CONTENT_STANDARDS.md`)

| Standard | Current draft (selective risk) | Evidence-sufficiency methodology |
|---|---|---|
| 1–2 references about the metric itself | Yes (El-Yaniv & Wiener 2010; Geifman & El-Yaniv 2017) | **No** peer-reviewed source exists |
| References are not governance frameworks | Yes | — |
| Implementation usable today, merged | Yes after merge (`src/metrology.rs`, `src/metrology_temporal.rs`) | Yes after merge (`src/evidence.rs`) |
| Implementation supports calculating the metric | Yes | Yes |
| Every link confirmed | Must be re-checked after merge and before any submission | Same |
| Required metadata, AI RMF characteristic, lifecycle stages | Yes (Valid & Reliable; Build, Operate & Monitor) | — |
| Concise definition, actionable guidance, scope boundaries | Yes, revised in Milestone 7 | Method document is long and research-stage |
| Adds value beyond the Metrology Center | Possibly: the visible catalog has Refusal Rate, Partial Refusal Rate and Test Fairness (Well-Calibration), but no selective-risk, conformal-coverage or shift entries. The value would be the established metric, not a TRIAD invention | Unestablished |

The draft was narrowed in Milestone 7 to the single quantity TRIAD measures
reliably. The earlier name, "validity-qualified selective error risk", implied an
added value that two real-data studies did not find. Evidence qualification is now
described as optional documentation with no demonstrated predictive value.

## Mechanical checks performed

Recorded in [`nist-schema-validation-v7.json`](nist-schema-validation-v7.json):

- Local validator (`scripts/validate-nist-draft.py`) and four negative cases: passed.
- `check-jsonschema 0.37.0` against the freshly fetched official schema: passed.
- Official `validation/validate_submission.py inspect` in a local copy of the
  `submissions/` directory: passed.
- A first wording was rejected by the official placeholder pattern
  (`<…>` text); the definition was rewritten in LaTeX.

```sh
python -m pip install "PyYAML==6.0.2" "jsonschema==4.23.0"
python scripts/validate-nist-draft.py
python scripts/validate-nist-draft.py --self-test
```

## Blockers and remediation

1. **Independent validation of evidence sufficiency.** Build a claim suite from real
   published reliability claims, with ground truth adjudicated by reviewers who did
   not write the rules (blinded to TRIAD's output), and pre-register the comparison
   against a strong checklist baseline (REFORMS-style) that is allowed to encode
   explicit scope and dependence rules.
2. **Decision utility.** Run a pre-specified study in which TRIAD's classifications
   change an outcome that matters, for example reviewer accuracy or time in auditing
   reliability claims, against an equally informed checklist.
3. **Peer review.** Publish the method; NIST reviewers expect references about the
   method itself.
4. **Unrelated-team replication** of the Bike Sharing and Electricity results with
   the reproduction package.
5. **Scope decision.** Either submit the established selective-risk metric with
   TRIAD as an implementation resource, which would need no TRIAD novelty claim, or
   wait until items 1–3 support a separate evidence-sufficiency methodology. One
   metric per submission.
6. **At submission time:** re-fetch the official schema and standards, re-run
   `check-jsonschema` and `validate_submission.py inspect`, confirm every link,
   remove research-stage wording from the YAML, and obtain explicit human approval.

## Public contact and process

Submitter: **Vers3Dynamics**. Contact: **ciao_chris@proton.me**. The address becomes
public if the draft is submitted. Submissions and review discussions on GitHub are
public and non-confidential. No NIST-facing pull request has been opened, and none
is authorised by this repository's ordinary development workflow.

Earlier assessments (Milestones 5 and 6) are preserved in the Git history of this
file and in [`nist-schema-validation.json`](nist-schema-validation.json) and
[`nist-schema-validation-v6.json`](nist-schema-validation-v6.json).
