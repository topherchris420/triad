# UCI Bike Sharing temporal benchmark (Milestone 7)

Second independent real-world benchmark: one-hour-ahead prediction of hourly
Capital Bikeshare rentals (Washington, D.C., 2011–2012). Results:
[Milestone 7 results](../../docs/metrology/milestone7-results.md).

**Source.** H. Fanaee-T, *Bike Sharing* [Dataset], UCI Machine Learning Repository,
2013, [doi:10.24432/C5W894](https://doi.org/10.24432/C5W894). Licensed CC BY 4.0
(as stated on the UCI dataset page). Associated publication: Fanaee-T & Gama,
*Progress in Artificial Intelligence* 2014,
[doi:10.1007/s13748-013-0040-3](https://doi.org/10.1007/s13748-013-0040-3).
`manifest.json` redistributes derived hourly fields (date, hour, holiday,
workingday, weekday, cnt) with this attribution. The raw archive is retrieved and
verified, not committed. TRIAD's MIT licence covers code, not the data.

| File | Role | Written |
|---|---|---|
| `data-audit.json` | Chronology, DST, missing elapsed hours, calendar consistency, 2011-only target summary | before fitting; 2012 targets unread |
| `uci-metadata.json` | UCI API response retrieved 2026-10-11 | before fitting |
| `protocol.json` | Frozen task, partitions, models, tolerance, policies, endpoints, statistics | before fitting (`bb1bd0f`) |
| `protocol-amendment-1.json` | Descriptive outcome-free comparison | before any metric (`edc06c6`) |
| `manifest.json` | `triad-temporal-regression-1`: hourly rows, settings, frozen predictions of both models | before any metric (`9c63076`) |
| `ridge-model.json`, `hgb-model.json` | Fitted states: coefficients, scalers, trees, error-scale and error-probability models | with the manifest |
| `provenance.json` | Hashes, software versions, eligibility counts | with the manifest |
| `frozen-before-v7.json` | SHA-256 of 102 pre-Milestone-7 artifacts | before results |
| `requirements.txt` | Pinned Python packages | — |

## Contract enforced by the Rust evaluator

`metrology benchmark` (schema `triad-temporal-regression-1`):

- Recomputes elapsed hours from wall-clock labels under the US 2007 DST rule. It
  rejects non-existent 02:00 labels, duplicate or out-of-order hours, weekday or
  workingday inconsistencies, and holiday flags that vary within a day.
- Recomputes lag eligibility and requires predictions for exactly the eligible
  development, calibration and test targets, in order.
- Requires contiguous chronological partitions; rejects unknown fields (for
  example an unsupported assurance flag).
- Reuses the generic selective evaluator via the normalised error
  u = (ŷ − y)/max(a, r·y), with Euclidean tolerance 1, and checks that its event
  count equals the material-error rule.
- Computes conformal coverage, every day-level policy and the TRIAD evidence
  decisions; matches budgets on calibration days; runs the paired circular
  moving-block bootstrap; applies the frozen decision rule.

A test-period count can never move a threshold, scale or rule (Rust test
`test_period_counts_cannot_move_fitted_quantities`). In this autoregressive task,
earlier test-period counts are both inputs and labels. Every decision is causal.

## Reproduce

See the [reproduction package](../../docs/metrology/milestone7-reproduction.md).
In short:

```sh
python -m pip install -r benchmarks/bike-sharing/requirements.txt
cargo run --release --locked -- metrology benchmark --input benchmarks/bike-sharing/manifest.json --output results/my-bike-sharing
cargo run --release --locked -- metrology validate --input results/my-bike-sharing/report.json
python scripts/prepare-bike-sharing.py --cache work/bike-cache --output work/bike-retrained
python scripts/check-bike-reproduction.py work/bike-retrained
```

## Known limitations

One city, one autumn test period. The tolerance and abstention cost are
conventions. Weather is excluded, so storms cannot be anticipated. Absent hours are
unknown (zero demand cannot be distinguished from an outage). Seasonal change is
not evidence of concept drift. Hours and days are dependent.
