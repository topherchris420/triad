# Claim-validity suite (Milestone 7, Experiment B)

`suite.json` holds 64 single-claim cases in the `triad-evidence-1` schema, each with
a predefined ground-truth class, the evidence dimension responsible, and a
rationale. `protocol.json` holds the endpoints and decision rule. Both were frozen
in commit `bb1bd0f`, together with the
[method](../../docs/metrology/evidence-sufficiency-method.md), before any
classifier existed.

| Source | Cases | Ground truth from |
|---|---:|---|
| `repository_record` | 13 | Recorded outcomes of Milestones 2–6 (for example the failed numerical gate, the M3 interval excluding a 2 pp effect, M6's unresolved chronology) |
| `constructed` | 47 | Verified dataset facts (for example `cnt = casual + registered` on all 17,379 rows) and explicit statistical criteria (Clopper–Pearson, Bartlett band, conformal exchangeability, leakage taxonomy) |
| `misleading_input` | 4 | Facts that the supplied evidence misstates; both methods are expected to fail |

Ground truth: 16 supported within scope, 1 not applicable, 28 insufficient
evidence, 19 contradicted.

**Disclosure.** The categories follow the Milestone 7 specification. The cases,
ground truth and TRIAD rules were all written by one author. Results on this suite
measure engineering detection capability and conformance to the specification,
not real-world validity. Independent cases, and challenges to individual labels,
are welcome through the
[scientific criticism template](../../.github/ISSUE_TEMPLATE/scientific-criticism.md).

```sh
cargo run --release --locked -- metrology evidence-suite --input benchmarks/evidence-validation/suite.json --output results/my-evidence-suite
cargo run --release --locked -- metrology validate --input results/my-evidence-suite/report.json
```

File-hash cases reference committed repository files. Run from the repository root
or pass `--root`.
