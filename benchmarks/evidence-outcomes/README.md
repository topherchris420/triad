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

Generator, comparators, runner and results are added in later commits, in the
order the protocol requires.
