# Selective-risk evaluation

Status: descriptive / exploratory.

Model: `trained-logistic-regression-v1`

Calibration-fixed threshold: 0.3549937151721641

| Scenario | Coverage | Selective risk | Abstention |
| --- | ---: | ---: | ---: |
| concept_shift | 0.8016666666666666 | 0.841995841995842 | 0.19833333333333336 |
| in_distribution | 0.8233333333333334 | 0.17408906882591094 | 0.17666666666666664 |

`null` means insufficient information, never zero risk.

Validity:
```json
{
  "interpretation": "descriptive_exploratory; input attestations are not independently verified by the evaluator",
  "missing_required_evidence": false,
  "numerical_status": "not_applicable",
  "out_of_evaluated_distribution": [
    "concept_shift"
  ],
  "scientifically_unsupported": true,
  "statistical_status": "statistically_underpowered_or_unestablished",
  "validated_physical_confidence": false
}
```

Evaluation seconds: 0.0743441

Bootstrap intervals condition on fitted model and calibration threshold, assume independent sampled clusters, and do not cover model discrepancy or unseen shift. Sequence and spatial intervals are separate sensitivity analyses, not a crossed-cluster guarantee. Individual uncertainty scores are distinct from population metric uncertainty.
