# NIST submission readiness: draft, not submission-ready

Reviewed October 11, 2026: [Center](https://airc.nist.gov/metrology/),
[submission guide](https://airc.nist.gov/metrology/how-to-submit/),
[review criteria](https://airc.nist.gov/metrology/how-to-submit/what-reviewers-look-for/),
[authoritative format](https://github.com/usnistgov/ai-metrology-submissions/blob/main/SUBMISSION_FORMAT.md).
Official schema source revision: `7f0609217621aa905973ad0cc03f5e6a8c97e345`.

The YAML uses supported schema version 1.0 and required identification,
definition, organization, contact, references and implementation fields.
`nist-schema-validation.json` records actual validation against the official
schema: **fails intentionally because a professional contact was not provided**.
No address was invented. No submission was sent.

- [x] Defined object and outputs: conditional excessive-error risk of a fixed model.
- [x] Grounded in directly relevant, verified selective prediction/calibration papers.
- [x] Documented loss, coverage, ties, sampling, missingness and distribution limits.
- [x] Candidate structured-data AI TEVV use: evaluate independently trained model outputs.
- [ ] Real public professional contact supplied by the submitting organization.
- [ ] Complete candidate passes the then-current official schema and link checks.
- [ ] Useful distinction from existing catalog methods independently assessed.
- [ ] Numerical adequacy and physical uncertainty established for sensor use.
- [ ] Independent pilot, utility-based effect size and adequately powered protocol.
- [ ] Independent replication and evidence beyond a synthetic transfer demonstration.
- [x] Implementation resource identifies runnable repository code in this change.
      Publication and CI status must be checked in the completion report.
- [ ] Human scientific review of application guidance and claimed contribution.

The current reasonable contribution is an honest reproducible evaluation
implementation with failure reporting. Catalog acceptance, novelty, NIST approval,
certification and endorsement are not claimed. Even catalog listing would not
establish endorsement or fitness for a particular application.
