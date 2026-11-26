# A writer-grouped real-data test for TRIAD certification

**Status: proposed, not preregistered or run.** This plan defines a candidate
cohort, a completed metadata-only inventory, and the gates that would have
to pass before running a frozen study. It does not certify a policy. It is separate
from the [Milestone 9 simulation](certification.md#milestone-9-the-pre-registered-simulation)
and from the evidence-reviewer comparison.

## Why writer groups

[NIST Special Database 19](https://www.nist.gov/srd/nist-special-database-19)
includes actual handprinted characters. Its [second-edition guide, Table 1 and
§2.2](https://s3.amazonaws.com/nist-srd/SD19/sd19_users_guide_edition_2.pdf)
documents segmented digit files grouped by writer and identifies a separately
collected school partition. Multiple digits by one writer give TRIAD dependent
observations inside a sequence without treating those observations as new
independent people.

Use only segmented digit crops for digit classification. Do not ingest full
pages, infer writer identity from handwriting, or publish crops or writer
identifiers. The writer number is a grouping key, not a feature. Use the
writer-preserving NIST archive, not a flattened train/test set. A public
archive does not itself prove independent sampling, common provenance across
partitions, or representativeness beyond its recorded collection.

## Gate 1: inventory and labels

Run the metadata-only audit against a local, checksum-verified copy of the
official second-edition `by_write.zip`. It reads ZIP member names but never
opens any image or label:

```sh
python scripts/audit-sd19-writers.py /path/to/by_write.zip > writer-inventory.json
python -m unittest discover -s tests -p test_sd19_audit.py
```

The audit plans the following disjoint writer roles:

| Role | Planned writer source |
| --- | --- |
| Development | `hsf_0`, `hsf_1` |
| Calibration and untouched in-cohort audit | `hsf_2`, `hsf_3`, `hsf_6`, `hsf_7`, `hsf_8`; split by the first byte of SHA-256 of `triad-sd19-writer-split-v1\|NNNN` (<128: calibration) |
| Descriptive collection shift | `hsf_4`; never used to fit, select thresholds, or claim certification |

All forms with the same four-digit writer number stay together. Count only
writers with at least 32 digit members. Require at least 800 planned writers
in each of calibration and in-cohort audit, plus nonempty development and shift
roles. Exit code 2 means that the inventory floor did not pass. This floor
prevents proceeding with an obviously undersized or misread archive; it is
**not** a power calculation or permission to open the audit outcomes.

The [official-archive inventory](../benchmarks/sd19/writer-inventory.json)
passed this floor: **995 development, 1,024 calibration, 1,052 in-cohort
audit, and 500 shift** writer IDs had at least 32 digit members. The
second-edition download returned no matching `hsf_8` digit members despite
the guide listing that partition. No images or labels were opened for this
inventory, and the counts can decrease after checking label linkage. The
record pins a digest of the matching member names, not of image contents.

On development only, verify that digit members can be linked to a NIST
reference label. One candidate is a conservative join to the official
`by_class` digit buckets using canonical decoded pixels: accept only a unique
class, log zero-match and conflicting-class exclusions, and spot-check against
an independent reference label before freezing the join. Do not infer class
from the image or filename sequence. If the join cannot be verified, stop and
resolve the data source rather than trying thresholds against ambiguous labels.
Count usable writers and predeclare the 32-digit selection and missing-label
rules without looking at model errors. The writer grouping must survive the
join. Hash case lists and data artifacts; report exclusions by role in
aggregate, with no writer IDs.

## Gate 2: freeze development choices

1. Fit a simple digit classifier on development writers only. A reasonable
   starting point is fixed resizing and multinomial logistic regression with
   risk score one minus the model's highest predicted probability. Freeze
   preprocessing, grouping, model coefficients, dependencies and the list of
   candidate score thresholds before scoring calibration or audit labels.
2. Use exactly 32 distinct eligible digits per writer with a deterministic
   selection rule that cannot use the score or correctness. The estimand is a
   uniformly selected digit in an equally weighted writer's sequence, for the
   specified collection. Preserve writer IDs for grouping, not for scoring.
3. Carry the Milestone 9 targets forward as proposed starting values: maximum
   accepted error 5%, minimum coverage 50%, family-wise budget 5%, and twenty
   thresholds. Before locking, use development writers to estimate the
   risk-coverage operating point, run `plan-certification` and simulate whole
   writers to assess power and audit precision. The CLI planner is plug-in,
   not an 80%-power calculation. If the available cohort is smaller
   than the design needs, stop or publish a descriptive pilot. Do not rescue
   the claim by widening targets after looking at held-out outcomes.
4. Record a versioned protocol, archive checksums and the fixed-source
   manifests before predictions on calibration or audit writers. Confirm
   separation by writer, duplicates across roles and data availability. If
   exchangeability within the target is not defensible, use descriptive
   evaluation only. Hash splitting prevents writer overlap but cannot
   establish that independence assumption.

## Gate 3: run once, then audit

- Compare legacy, Hoeffding–Bentkus and betting on identical calibration
  candidates and writer sequences. Predeclare Hoeffding–Bentkus as the sole
  candidate for the primary certificate claim; the alternatives are labelled
  comparisons. Do not choose whichever method certifies at the same budget.
  The existing [method and verifier](certification.md#verification) control
  the implementation. Retain refusals and candidate-level decisions.
- Apply each fixed selected policy to the untouched in-cohort audit without
  refitting. Report both acceptance coverage and errors conditional on
  acceptance, accepted counts, and writer-level uncertainty that preserves
  pairing and dependence within a writer. Zero accepted predictions imply
  undefined accepted-error risk, not zero error. Give the comparable
  unconditional classifier error and coverage too.
- **Evidence decision:** if the primary method refuses, the independent audit
  is too imprecise, a target-population assumption fails, or the frozen
  audit bounds cannot substantiate both targets, report **inconclusive**
  for real-data support. If the audit establishes a prespecified violation,
  report **not supported**. A pass is scoped to the declared writer cohort
  and model. A single real-data calibration does not estimate a
  certification rate across repeat studies or reproduce the simulation's
  claimed sample-size ratio.
- Report `hsf_4` separately as a descriptive shift check with the same
  frozen policy. A pass there does not establish a guarantee for a
  school population. No sensor or hardware conclusions follow.

The study refers to [Learn then Test](https://arxiv.org/abs/2110.01052) for
the multiplicity discipline and [Waudby-Smith and Ramdas](https://arxiv.org/abs/2010.09686)
for the bounded-mean betting basis. Neither paper waives the need to check how
these writer groups were collected or to keep the audit independent of model and
threshold selection.
