# Milestone 9: certification of abstention policies

Simulation under the frozen protocol `experiments/certify-v1.json`. Certification rate / false-certification rate / mean true coverage of the selected threshold (refusal counts as zero coverage).

| Error model | Regime | Sequences | Legacy Hoeffding ratio | Hoeffding–Bentkus | Betting | Naive frame binomial |
|---|---|---:|---|---|---|---|
| graded | independent_frames | 100 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 |
| graded | independent_frames | 250 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.014 / 0.000 / 0.010 |
| graded | independent_frames | 500 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.012 / 0.000 / 0.008 | 0.105 / 0.000 / 0.067 |
| graded | independent_frames | 1000 | 0.000 / 0.000 / 0.000 | 0.023 / 0.000 / 0.014 | 0.325 / 0.000 / 0.202 | 0.452 / 0.000 / 0.284 |
| graded | independent_frames | 2500 | 0.000 / 0.000 / 0.000 | 0.633 / 0.000 / 0.391 | 0.958 / 0.000 / 0.618 | 0.985 / 0.000 / 0.654 |
| graded | independent_frames | 5000 | 0.000 / 0.000 / 0.000 | 1.000 / 0.000 / 0.669 | 1.000 / 0.000 / 0.693 | 1.000 / 0.000 / 0.706 |
| graded | independent_frames | 10000 | 0.000 / 0.000 / 0.000 | 1.000 / 0.000 / 0.716 | 1.000 / 0.000 / 0.727 | 1.000 / 0.000 / 0.733 |
| graded | independent_frames | 25000 | 1.000 / 0.000 / 0.636 | 1.000 / 0.000 / 0.748 | 1.000 / 0.000 / 0.749 | 1.000 / 0.000 / 0.751 |
| graded | dependent_frames | 100 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.471 / 0.001 / 0.299 |
| graded | dependent_frames | 250 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.982 / 0.000 / 0.651 |
| graded | dependent_frames | 500 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 1.000 / 0.000 / 0.707 |
| graded | dependent_frames | 1000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.964 / 0.000 / 0.589 | 1.000 / 0.000 / 0.732 |
| graded | dependent_frames | 2500 | 0.000 / 0.000 / 0.000 | 0.850 / 0.000 / 0.491 | 1.000 / 0.000 / 0.733 | 1.000 / 0.000 / 0.751 |
| graded | dependent_frames | 5000 | 0.000 / 0.000 / 0.000 | 1.000 / 0.000 / 0.669 | 1.000 / 0.000 / 0.750 | 1.000 / 0.000 / 0.754 |
| graded | dependent_frames | 10000 | 0.000 / 0.000 / 0.000 | 1.000 / 0.000 / 0.703 | 1.000 / 0.000 / 0.755 | 1.000 / 0.000 / 0.760 |
| graded | dependent_frames | 25000 | 1.000 / 0.000 / 0.647 | 1.000 / 0.000 / 0.750 | 1.000 / 0.000 / 0.770 | 1.000 / 0.000 / 0.780 |
| null | independent_frames | 100 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 |
| null | independent_frames | 250 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 |
| null | independent_frames | 500 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 |
| null | independent_frames | 1000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 |
| null | independent_frames | 2500 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.001 / 0.001 / 0.001 | 0.000 / 0.000 / 0.000 |
| null | independent_frames | 5000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 |
| null | independent_frames | 10000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 |
| null | independent_frames | 25000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 |
| null | dependent_frames | 100 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.001 / 0.001 / 0.001 |
| null | dependent_frames | 250 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.001 / 0.001 / 0.001 |
| null | dependent_frames | 500 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 |
| null | dependent_frames | 1000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 |
| null | dependent_frames | 2500 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 |
| null | dependent_frames | 5000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 |
| null | dependent_frames | 10000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 |
| null | dependent_frames | 25000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 / 0.000 |

## Pre-registered hypotheses

```json
{
  "E1_exploratory": {
    "naive_exceeds_delta": false,
    "null_dependent_cells": [
      {
        "false_certification_lower_99": 0.00001,
        "false_certification_rate": 0.001,
        "sequences": 100
      },
      {
        "false_certification_lower_99": 0.00001,
        "false_certification_rate": 0.001,
        "sequences": 250
      },
      {
        "false_certification_lower_99": 0.0,
        "false_certification_rate": 0.0,
        "sequences": 500
      },
      {
        "false_certification_lower_99": 0.0,
        "false_certification_rate": 0.0,
        "sequences": 1000
      },
      {
        "false_certification_lower_99": 0.0,
        "false_certification_rate": 0.0,
        "sequences": 2500
      },
      {
        "false_certification_lower_99": 0.0,
        "false_certification_rate": 0.0,
        "sequences": 5000
      },
      {
        "false_certification_lower_99": 0.0,
        "false_certification_rate": 0.0,
        "sequences": 10000
      },
      {
        "false_certification_lower_99": 0.0,
        "false_certification_rate": 0.0,
        "sequences": 25000
      }
    ]
  },
  "H1_validity": {
    "exceedances": [],
    "supported": true
  },
  "H2_dominance": {
    "supported": true,
    "violating_datasets": 0
  },
  "H3_primary_efficiency": {
    "legacy_n80": 25000,
    "legacy_n80_note": "reached within grid",
    "method": "hoeffding-bentkus",
    "n80": 2500,
    "supported": true
  },
  "H4_secondary_efficiency": {
    "legacy_n80": 25000,
    "legacy_n80_note": "reached within grid",
    "method": "betting",
    "n80": 1000,
    "supported": true
  }
}
```

This is a simulation of statistical procedures. It does not validate a sensor, a scoring model or any deployment.
