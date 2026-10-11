# Bike Sharing temporal benchmark (Milestone 7)

Exploratory external-data evaluation; hours and days are dependent, not independent trials. Every count, curve, interval and test day is in report.json.

| Model | Test MAE (bikes) | Material-error rate | Selective coverage | Selective risk | Conformal 90% coverage | Primary delta cost (TRIAD - std) | 97.5% interval (7-day blocks) | Decision |
|---|---:|---:|---:|---:|---:|---:|---|---|
| ridge | 32.84 | 0.0969 | 0.8236 | 0.0991 | 0.8795 | 0.00490 | [0.00059, 0.01052] | evidence_insufficient_to_establish_a_practical_advantage |
| hgb | 51.18 | 0.2103 | 0.8190 | 0.1488 | 0.8999 | 0.00624 | [0.00142, 0.01224] | evidence_insufficient_to_establish_a_practical_advantage |

Added predictive utility established for both models: false.
Positive delta means TRIAD's day-level decisions cost more than standard monitoring at the matched calibration budget. Null means insufficient information, never zero risk.
