# NIST AI Metrology Center: selective-risk submission package

**Status: submitted October 11, 2026** as
[usnistgov/ai-metrology-submissions#24](https://github.com/usnistgov/ai-metrology-submissions/pull/24),
through the NIST AI Metrology Center's
[community submission process](https://github.com/usnistgov/ai-metrology-submissions).
The file
[`submissions/selective-risk-at-target-coverage.yml`](submissions/selective-risk-at-target-coverage.yml)
is byte-identical to the file in that pull request (head `ff442fd`) and is laid out
exactly as it sits in the fork. The pull request was not merged as of October 13,
2026; NIST reviewers decide. Revision in place on the pull request is the normal
path, and any change made there must also be made here so the two copies stay
identical. See [Submission gate](#submission-gate) for where each pre-submission
item stands.

## What is being submitted, and what is not

This package submits **the established metric**: selective risk at a target coverage,
from the selective-classification literature (El-Yaniv & Wiener 2010; Geifman &
El-Yaniv 2017). TRIAD appears only as one implementation resource, alongside an
independent released library (TorchUncertainty 0.13.0, `RiskAtxCov`). Neither
implementation is claimed to be novel.

Deliberately **excluded**:

- TRIAD's claim-relative evidence-sufficiency method. Its independent,
  outcome-grounded test ([Milestone 8](../milestone8-results.md)) found it no
  better than blind strong comparators, and it has no peer-reviewed reference. It
  is not to be proposed.
- TRIAD's Milestone 6/7 research findings and validity annotations. A catalog entry
  describes a metric, not one implementation's research programme.

The earlier [prototype draft](../nist-submission-draft.yml) ("selective
excessive-error risk at calibration-fixed coverage", Milestone 7) is kept unchanged
as a historical record. This candidate supersedes it: it uses the literature's
notation and name, the general bounded loss, concise scope boundaries, and drops
TRIAD-specific research wording.

## Official standards used

Fetched October 11, 2026 from `usnistgov/ai-metrology-submissions` at commit
`7f0609217621aa905973ad0cc03f5e6a8c97e345` (unchanged since Milestone 7). Files used:
`SUBMISSION_FORMAT.md`, `CONTENT_STANDARDS.md`, `CONTRIBUTING.md`,
`.github/PULL_REQUEST_TEMPLATE.md` and `validation/schemas/v1.json`. The schema is
byte-identical to the vendored [`nist-schema-v1.json`](../nist-schema-v1.json)
(SHA-256 `f815015d…da7c`). The AIRC pages
[What reviewers look for](https://airc.nist.gov/metrology/how-to-submit/what-reviewers-look-for/)
render the same `CONTENT_STANDARDS.md` (last read by AIRC 2026-10-09).

## Mechanical checks (passed)

| Check | Result |
|---|---|
| `validation/validate_submission.py inspect --files submissions/selective-risk-at-target-coverage.yml`, official script in a local clone with the file committed | **Passed** |
| `check-jsonschema 0.38.2 --schemafile validation/schemas/v1.json` (the exact command the repository's CI runs) | **Passed** |
| `python scripts/validate-nist-draft.py` and `--self-test` (vendored schema; 4 malformed variants rejected) | **Passed** |
| Placeholder guard (no `<...>` text), single file, lowercase-hyphen file name | **Passed** |
| Re-run October 13, 2026 on the submitted file (PR head `ff442fd`) against the official repository, still at `7f06092`: `validate_submission.py inspect` and `check-jsonschema 0.38.2` | **Passed** |

## Content standards self-check

Each item from the official `CONTENT_STANDARDS.md`, with the evidence for it.

### Scientific grounding & evidence

| Standard | Status | Evidence |
|---|---|---|
| 1–2 references about the metric itself | ✅ | El-Yaniv & Wiener (2010) define coverage `Φ(f,g)=E[g(X)]` (Definition 1) and selective risk as mean bounded loss on accepted inputs (Definition 2), on p. 1606, confirmed in the publisher PDF. Geifman & El-Yaniv (2017) fix a threshold so that a desired risk holds at test time, confirmed in the NeurIPS abstract. A third reference (Künsch 1989) grounds the block-bootstrap interval recommended for dependent data |
| Specific, peer-reviewed | ✅ | JMLR; NeurIPS proceedings; Annals of Statistics |
| Not a governance framework | ✅ | None cited |
| Implementation usable today: released, or merged into an open-source repository | ✅ | `src/metrology.rs` is on `master`. TorchUncertainty 0.13.0 is released on PyPI (Apache-2.0), and `RiskAtxCov` was inspected in the wheel. The quickstart page has been on `master` since the merge on October 11, 2026 (`7be64c8`) |
| Implementation supports calculating the metric | ✅ | `metrology evaluate` on `examples/predictions.json` returns `coverage`, `selective_error_risk`, `accepted` and `accepted_errors` under `report.overall.operational`. TorchUncertainty `RiskAtxCov` computes the retrospective (matched-coverage) variant. Since October 13, 2026 the quickstart also gives a standard-library Python function for the calibration-fixed form; CI requires it to match `metrology evaluate` exactly ([`check-quickstart-reference.py`](../../../scripts/check-quickstart-reference.py)) |
| Every link confirmed: not broken, moved or redirecting | ✅ 6/6 | [`link-check.json`](link-check.json), regenerated October 13, 2026: all six links return HTTP 200 with no redirect. The pre-merge record (October 11) had the quickstart at 404 because it was not yet on `master`; it is kept in Git history. The Künsch DOI redirects, so the final Project Euclid URL is used instead |

### AI RMF alignment & metadata

| Standard | Status | Evidence |
|---|---|---|
| Required fields | ✅ | All seven, plus optional context fields |
| AI RMF characteristic | ✅ | **Valid & Reliable**: the metric measures how often accepted outputs are wrong |
| Lifecycle stages | ✅ | **Build** (model and score comparison), **Deploy** (fixing the threshold on calibration data), **Operate & Monitor** (accepted-error rate over time) |

### Usage guidance & utility

| Standard | Status | Evidence |
|---|---|---|
| English | ✅ | |
| Concise definition | ✅ | Coverage, risk and the calibration-threshold rule only; conventions live in the linked quickstart |
| Actionable guidance on applying and interpreting it | ✅ | `usage_details`: when to use it, compare at matched coverage, which interval to report |
| Scope boundaries stated | ✅ | `usage_details` "Scope:" sentence; `known_failure_modes` |
| Adds value beyond the Metrology Center | ✅ | The live catalog (147 entries, read October 11, 2026) has no selective-risk, risk–coverage, abstention or conformal entry. The nearest entries, *Refusal Rate*, *Partial Refusal Rate*, *False Refusal Rate* and *Test Fairness (Well-Calibration)*, measure how often a system declines, or calibration across groups, not the error of accepted outputs. Among open submissions, one (usnistgov/ai-metrology-submissions#9, a composite runtime score) lists AURC only as a related metric |

**Readiness estimate per `CONTENT_STANDARDS.md`:** the one unchecked item before
merge was the quickstart link. The merge fixed it and the regenerated
`link-check.json` records it, so every item is checked, which points to
**"Accept"**. This is an estimate; NIST reviewers decide.

## Submission gate

The gate as written before submission, and where each item stands now. The pull
request was opened on October 11, 2026.

1. This branch is merged to `master` and CI is green.
   *Merged October 11, 2026 (`7be64c8`).*
2. `python scripts/check-nist-links.py --output docs/metrology/nist-submission/link-check.json`
   exits 0 (all six links return 200 with no redirect).
   *Not done before the pull request was opened; the record then predated the merge.
   Re-run October 13, 2026: exits 0, all six links 200 with no redirect.*
3. The official repository is re-fetched. If `SUBMISSION_FORMAT.md`,
   `CONTENT_STANDARDS.md` or `validation/schemas/v1.json` changed, re-run both
   official checks.
   *Re-fetched October 13, 2026: unchanged at `7f06092`. Both official checks pass on
   the submitted file.*
4. A person at Vers3Dynamics has read the YAML and confirms that the contact
   address `ciao_chris@proton.me` may become public.
   *The address is now public on the pull request.*
5. That person opens the pull request from their own fork, using the body below,
   and follows it for review comments.
   *Opened from the Vers3Dynamics GitHub account as
   [#24](https://github.com/usnistgov/ai-metrology-submissions/pull/24).*

## Pull-request body (pre-filled from the official template)

This is the body prepared before submission. The description on
[#24](https://github.com/usnistgov/ai-metrology-submissions/pull/24) is the
authoritative version.

```markdown
## Metric submission

Selective risk at target coverage: the error rate among the predictions a model
chooses to answer, reported at a stated coverage with the abstention threshold
fixed on calibration data. It is the standard measure for evaluating
classifiers and regressors with a reject option.

## Checklist

- [x] This PR adds **one** YAML file under `submissions/`, following SUBMISSION_FORMAT.md.
- [x] All required fields are filled in (`schema_version`, name, applied definition, submitter organization(s), contact email, references, implementation resources).
- [x] The submission contains **no proprietary or confidential information** --- I understand everything posted here is public and non-confidential.
- [x] I will follow this pull request for review comments (GitHub email notifications enabled on my account).
- [x] I have self-checked my submission against CONTENT_STANDARDS.md --- the substantive criteria reviewers apply.

## Additional context

- **Maturity:** long-established (Chow 1970; El-Yaniv & Wiener 2010; Geifman &
  El-Yaniv 2017). We propose the existing metric, not a new one.
- **Relation to the catalog:** no current entry measures the error of accepted
  outputs. The Refusal Rate entries count how often a generative system
  declines; Test Fairness (Well-Calibration) concerns calibration. AURC and
  coverage-at-target-risk are listed as variants rather than separate
  submissions (one metric per PR).
- **Implementations:** TorchUncertainty (`RiskAtxCov`, retrospective variant) and
  the open-source TRIAD tool (calibration-fixed threshold, block/cluster
  bootstrap intervals). The submitter maintains TRIAD.
- **Open question for reviewers:** would you prefer the retrospective
  matched-coverage form (common in benchmarks) as the primary definition, with
  the calibration-fixed threshold as a variant? We chose the calibration-fixed
  form because it is the deployable one.
```
