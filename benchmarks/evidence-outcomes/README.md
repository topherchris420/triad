# Milestone 8: outcome-grounded validation of evidence sufficiency

Milestone 7 tested TRIAD's claim-relative evidence rules on 64 cases whose rules,
cases and ground truth shared one author. Detection there showed engineering
capability, not validity. This benchmark removes that circularity:

- **Truth comes from the world.** Every claim is about a finite, real population of
  predictions (a period, month, batch or week). Its truth is computed from held-out
  outcomes, sealed away from every classifier.
- **New data.** Five public CC BY 4.0 datasets that TRIAD has never used: Beijing air
  quality, I-94 traffic, household power, gas-sensor drift and bank marketing.
- **Strong comparators, designed blind.** A REFORMS-style checklist reviewer (R) and
  a dependence-aware statistician (S) are written from
  [`comparator-brief.md`](comparator-brief.md) alone, without access to TRIAD's
  rules. Floors: never support, standard checks C, and C+scope.
- **One pre-registered decision rule** (family-balanced decision cost), in
  [`protocol.json`](protocol.json).

| File | Role | Frozen |
|---|---|---|
| `protocol.json` | Datasets, periods, models, claim families, templates, endpoints, decision rule | Before any model, case or comparator existed |
| `comparator-brief.md` | The only input given to the comparator author | Same commit |
| `frozen-before-v8.json` | SHA-256 of the Milestone 7 evidence and the evidence-engine sources | Same commit; enforced by `tests/research_integrity.rs` |
| `implementation-notes.md`, `data-audit.json` | Choices the protocol left open; eligibility counts (no model) | Before any model was fitted on real data |
| `comparators/` | Blind comparators R and S, byte-identical to delivery | Before any real case |

## Result

**Not established** (pre-registered verdict). Across 5,078 claims, TRIAD's
family-balanced decision cost was 0.159. That is worse than the blind statistician
S (0.121; Δ = +0.038 [+0.008, +0.070]), indistinguishable from the blind checklist
R (0.160), and worse than never endorsing (0.125). TRIAD does beat standard checks
C (0.331) and C+scope (0.209). Details, analyses and limitations are in
[`docs/metrology/milestone8-results.md`](../../docs/metrology/milestone8-results.md).
Archived outputs are in [`results/evidence-outcomes-v8/`](../../results/evidence-outcomes-v8/).

## Reproduce

```sh
python -m pip install -r benchmarks/evidence-outcomes/requirements.txt
python scripts/m8-generate.py --cache work/m8-cache --output work/m8
cargo run --release --locked -- metrology evidence-batch --input work/m8/cases.json --output work/m8/batch
cp work/m8/batch/classifications.json work/m8/
python benchmarks/evidence-outcomes/comparators/m8_comparators.py --input work/m8/cases.json --output work/m8/comparators.json
python scripts/m8-score.py --truth work/m8/truth.json --triad work/m8/classifications.json --comparators work/m8/comparators.json --output work/m8/score
python scripts/verify-milestone8.py --run work/m8
```

The generator downloads the five UCI archives (about 40 MB) and verifies their
SHA-256 against the protocol.
