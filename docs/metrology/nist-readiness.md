# NIST AI Metrology Center: readiness assessment (Milestone 8)

**Status (October 13, 2026):** the established selective-risk metric was submitted
on October 11, 2026 as
[usnistgov/ai-metrology-submissions#24](https://github.com/usnistgov/ai-metrology-submissions/pull/24),
after this branch was merged. It was not merged by NIST as of October 13, 2026.
Nothing else has been submitted.

**Recommendation:**

- **Submit the established selective-risk metric.** Done (see status above). TRIAD
  appears only as one implementation. Where each
  [submission gate](nist-submission/README.md#submission-gate) item stands is
  recorded in the package.
- **Do not propose TRIAD's evidence-sufficiency method.** An independent,
  outcome-grounded test against blind strong comparators did not support it.

Catalog inclusion, if it ever occurs, does not imply NIST endorsement.

Assessed October 11, 2026 against the official sources:
[AI Metrology Center](https://airc.nist.gov/metrology/),
[how to submit](https://airc.nist.gov/metrology/how-to-submit/),
[what reviewers look for](https://airc.nist.gov/metrology/how-to-submit/what-reviewers-look-for/),
and the [submissions repository](https://github.com/usnistgov/ai-metrology-submissions)
(`SUBMISSION_FORMAT.md`, `CONTENT_STANDARDS.md` and `validation/schemas/v1.json` at
commit `7f06092`, re-fetched for Milestone 8 and unchanged; the schema is
byte-identical to the vendored [`nist-schema-v1.json`](nist-schema-v1.json)). NIST's
process is public and iterative: reviewers comment on the pull request and the
submitter revises in place.

## Two candidates, two decisions

| | Selective risk at target coverage | Claim-relative evidence sufficiency |
|---|---|---|
| What it is | The established metric (El-Yaniv & Wiener 2010; Geifman & El-Yaniv 2017): error rate among accepted predictions at a stated coverage, with the threshold fixed on calibration data | TRIAD's rule system classifying whether typed evidence supports a typed reliability claim |
| Novelty claimed | None. TRIAD is one implementation, alongside TorchUncertainty | Would have been TRIAD's only distinctive contribution |
| Scientific grounding | Peer-reviewed references about the metric itself | No peer-reviewed reference. Milestone 7 test was circular (single-author suite) |
| Independent validation | Not needed for an established metric | **Milestone 8 failed it:** worse than a blind statistical reviewer (Δ cost +0.038 [+0.008, +0.070]), indistinguishable from a blind REFORMS checklist, and worse than never endorsing |
| Catalog gap | Yes: no selective-risk, risk–coverage, abstention or conformal entry among the 147 published (re-read October 13, 2026: unchanged) | — |
| Official checks | `validate_submission.py inspect` and `check-jsonschema 0.38.2` pass (re-run on the submitted file October 13, 2026) | — |
| Decision | **Submitted** as [#24](https://github.com/usnistgov/ai-metrology-submissions/pull/24) ([package](nist-submission/README.md)) | **Not to be proposed** |

## What Milestone 8 showed about evidence sufficiency

[Full results](milestone8-results.md). The test covered 5,078 claims on five public
datasets new to TRIAD, each about a finite real population (a period, month,
batch or week). Truth came from held-out outcomes no classifier saw. The rules,
protocol, generator and comparators were frozen before any case existed.

- **No distinctive value.** A statistical reviewer and a REFORMS-style checklist,
  both written by an agent that never saw TRIAD's rules, matched or beat it. Its
  support decisions agree with each on 93% of claims.
- **A replicated but generic advantage.** TRIAD beats plain standard checks
  (Δ = −0.172) and checks with scope (Δ = −0.050) with world truth. That is the
  Milestone 7 direction, but it is ordinary good practice that strong reviewers
  also implement.
- **A specific weakness.** For next-week operational claims TRIAD has no predictive
  bound, and it endorsed 7 of 19 false ones.
- **A protocol lesson.** The pre-registered "share of false claims endorsed ≤ 0.05"
  criterion is not guaranteed by valid methods when bounds are derived from the
  same sample. Future protocols should use P(endorsed and false). This does not
  change the verdict.

## Three stages of readiness (updated)

| Stage | Selective-risk metric | Evidence-sufficiency method |
|---|---|---|
| 1. Engineering ready | **Met**: implementation on `master`, deterministic replay, independent verifiers, Linux and Windows CI | Met |
| 2. Scientifically reviewable | **Met**: established, peer-reviewed metric; TRIAD's real-data use is documented (Milestones 6–7) | Met as an object of study: frozen protocols and fully reported negative results |
| 3. Compelling catalog candidate | **Likely**: submitted as #24 after the merge, and the quickstart link now resolves. NIST reviewers decide | **Not met**, and Milestone 8 is evidence against it |

## Selective-risk submission: steps and status

1. Merge this branch with green CI. *Merged October 11, 2026 (`7be64c8`).*
2. Run `python scripts/check-nist-links.py`; all six links must return 200 without
   redirect. *Done October 13, 2026, after the pull request was opened: all six
   return 200 ([`link-check.json`](nist-submission/link-check.json)).*
3. Re-fetch the official repository and re-run both official checks if anything
   changed. *Done October 13, 2026: still at `7f06092`, and both checks pass on the
   submitted file.*
4. A person at Vers3Dynamics reviews the YAML, confirms the public contact
   **ciao_chris@proton.me**, and opens the pull request from their own fork with the
   [prepared body](nist-submission/README.md#pull-request-body-pre-filled-from-the-official-template).
   *Opened October 11, 2026 as
   [#24](https://github.com/usnistgov/ai-metrology-submissions/pull/24) from the
   Vers3Dynamics account.*
5. Answer reviewer comments on the pull request. Revision in place is the normal
   path, and NIST says a reviewer usually picks a submission up within two weeks.
   Mirror any revision in
   [`nist-submission/submissions/selective-risk-at-target-coverage.yml`](nist-submission/submissions/selective-risk-at-target-coverage.yml).

## If evidence sufficiency is ever to be reconsidered

Any repaired rule set must be frozen and then tested on **new** data and **new**
claims. Re-scoring Milestone 8 after changing rules would be tuning to the test.
The repairs to test would be:

- a predictive bound for operational claims;
- design-effect estimation from raw ordered outcomes;
- re-examining the completeness and representativeness gates.

It would also need:

- a human-adjudicated suite of real published claims (leakage, provenance and
  attestation rules remain untested against the world);
- a reviewer-utility study;
- unrelated-team replication;
- a peer-reviewed publication about the method itself.

## Public contact and process

Submitter: **Vers3Dynamics**. Contact: **ciao_chris@proton.me**, which is now
public on the pull request. Submissions and review discussions on GitHub are public
and non-confidential. The only NIST-facing pull request is
[#24](https://github.com/usnistgov/ai-metrology-submissions/pull/24). No other is
authorised by this repository's ordinary development workflow.

Earlier assessments (Milestones 5–7) are preserved in the Git history of this file,
in [`nist-schema-validation.json`](nist-schema-validation.json),
[`nist-schema-validation-v6.json`](nist-schema-validation-v6.json) and
[`nist-schema-validation-v7.json`](nist-schema-validation-v7.json), and in the
historical [Milestone 7 draft](nist-submission-draft.yml).
