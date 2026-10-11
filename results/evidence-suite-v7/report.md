# Claim-validity suite (Experiment B)

Author-constructed suite; engineering detection capability only.

| Method | Detection of unsupported claims | False rejection of valid claims | Four-class accuracy | Indeterminate | Audit completeness |
|---|---:|---:|---:|---:|---:|
| TRIAD | 43/47 | 0/16 | 60/64 | 27 | 43/43 |
| Standard checks C | 20/47 | 0/16 | 34/64 | 8 | 17/20 |
| C + scope rule | 26/47 | 0/16 | 40/64 | 14 | 23/26 |

Primary difference 0.48936170212765956 (threshold 0.2), criterion met: true. Exact McNemar (suite-internal) p = 2.3841857910156245e-7.

See report.json for every case, reason, ablation and discordant-case attribution.
