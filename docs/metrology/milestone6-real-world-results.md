# Milestone 6: real-world comparative metrology

**Executed October 11, 2026. No detectable added warning utility in this experiment; insufficient evidence for a general superiority claim.** The engineering implementation and computational verification succeeded. The prespecified scientific added-value hypothesis did not survive this dataset/model comparison.

## Study and provenance

Vers3Dynamics, PI Christopher Woodyard. Starting master `ea15d40`; protocol committed as `3f3046f` before fitting or summarizing final outcomes. See [architecture audit](milestone6-architecture-audit.md), [frozen protocol](../../benchmarks/electricity/protocol.json), [provenance](../../benchmarks/electricity/provenance.json), [input manifest](../../benchmarks/electricity/manifest.json) and [full report](../../results/benchmark-electricity/report.json). No final-test model/threshold selection was performed. This is an exploratory local protocol, not independent preregistration.

[OpenML Electricity ID 151, version 1](https://www.openml.org/search?id=151&type=data) supplies 45,312 NSW electricity observations. The [authoritative metadata](https://www.openml.org/api/v1/json/data/151) identifies Harries, Gama and Bifet; class UP/DOWN compares NSW price to its preceding 24-hour moving average. Source SHA-256 is `2d86fbc74c69a5c04bcde0119bce280dd6d8efa0115987edc75b77ccf024d7f2`, MD5 `8ca97867d960ae029ae3a9ac2c923d34`. The source calls its license “Public”; no specific license is inferred. Raw ARFF is retrieved, not redistributed. Source metadata and derived model/evaluation artifacts are retained.

The published file has eight features plus class, rather than the description's inconsistent five-field count. We found no missing released values, but its ARFF relation explicitly declares normalization and missing-value replacement. Original missingness and global preprocessing cannot be reconstructed. The normalized date moves backwards at rows 25,488, 34,896, 35,232, 36,240 and 40,704. There are 933 unique dates across 944 48-period blocks. Periods and weekdays cycle continuously. We preserve published row order; **calendar chronology is unresolved**, and all temporal interpretation is conditional on that order. We do not fabricate dates or sort by a corrupted field. The claimed May 1996–December 1998 span is source metadata, not independently verified timestamps.

Known current weekday/period are represented by sine/cosine pairs. Five prior-day (48-row-lag) fields supply NSW/Victoria price, demand and transfer. Current price directly constructs the label and is excluded; current demand/transfer and normalized date are also excluded. No target lag is used. StandardScaler fits training only. Earlier observations can supply lagged features to a later split, as in forecasting; no later observation supplies an earlier predictor. Training and calibration targets still share temporal dependence across nearby observations, which disjoint row IDs alone do not remove. Upstream full-file normalization/imputation prevents a strong claim of entirely leakage-free preprocessing.

| Split | Published day indices, half-open | Rows | Role |
|---|---|---:|---|
| Training | [1,470) | 22,512 | Fit models and scaler |
| Development | [470,610) | 6,720 | Reference feature/score means and diagnostic scales |
| Calibration | [610,750) | 6,720 | Acceptance and warning thresholds |
| Test | [750,944) | 9,312 | Untouched final period for outcome evaluation |

Day 0 is excluded because a prior-day feature is unavailable. Test comprises 27 nonoverlapping seven-day windows and a final five-day window. Calibration comprises 20 seven-day windows. The models use identical features/splits: L2 logistic regression (C=1, lbfgs, max_iter=1000, seed61001) and random forest (200 trees, depth8, leaf minimum20, sqrt feature sampling, one worker, seed61002). No hyperparameter search occurs. Logistic fit converged within its limit. Pinned NumPy/SciPy/scikit-learn versions, coefficients, tree structure, scaler and hashes are exported. Clean retraining is checked separately from Rust metric replay.

## Conventional and selective results

Classification loss is 0/1 with unacceptable-error tolerance zero. Uncertainty and error probability are `min(P(UP),1-P(UP))`; Brier/ECE refer to the **prediction-error event**, not an undocumented positive-class probability. Equal-width ECE uses ten bins. Calibration fixes nominal 80% acceptance; all boundary ties are included. AURC is the right-endpoint integral over complete tied-score groups, evaluated retrospectively.

| Test result | Logistic | Forest |
|---|---:|---:|
| Accuracy, all predictions | 65.5713% | 66.0009% |
| Total errors / observations | 3,206 / 9,312 | 3,166 / 9,312 |
| Acceptance threshold | 0.416439754647 | 0.386620838738 |
| Accepted errors / accepted | 2,246 / 7,202 | 1,455 / 5,630 |
| Actual selective coverage | 77.3411% | 60.4596% |
| Selective risk | 31.1858% | 25.8437% |
| Abstention frequency | 22.6589% | 39.5404% |
| Incorrect acceptance / all observations | 24.1194% | 15.6250% |
| AURC | 0.302209 | 0.212384 |
| Event Brier | 0.224694 | 0.207131 |
| Event ECE | 0.032842 | 0.017306 |
| Seven-day-block risk interval, descriptive 95% | [28.3796%,34.1146%] | [22.9246%,29.2352%] |

Forest conditional risk is lower at **substantially lower actual coverage**. Its shared-day paired difference versus logistic is -5.3421 percentage points, descriptive seven-day interval [-7.1517,-3.6690]. This compares each model's own accepted set and is not a matched-coverage superiority result. Full curves and retrospective matched-coverage statistics are retained to avoid hiding that distinction.

Calibration accepted risk was 35.7329% logistic and 37.2954% forest. Final aggregate risk is lower, not a uniform degradation. Across later windows selective risk spans 17.24–46.21% and 12.39–45.13%, respectively; window ECE spans 0.02944–0.21578 and 0.03300–0.17762. Aggregates conceal meaningful heterogeneity. Observable feature/score shifts and retrospective performance/calibration changes do **not** establish a change in P(Y|X) or identify a causal concept-drift event.

## Falsifiable warning comparison and ablations

Covariate score is the largest absolute change in standardized feature means relative to development. Confidence score is the absolute change in mean error probability. Each is scaled by its development-window 80th percentile, with a 1e-12 floor. Combined score is their maximum. Each policy threshold is the calibration-window 80th percentile and alerts strictly above it. All policies produced **4/20 calibration alerts**, a matched calibration budget; test alert budgets are allowed to diverge and are shown. Diagnostics use no test labels. Retrospective degradation means window accepted risk exceeds calibration risk by >5 percentage points.

The hypothesis required greater degraded-window recall than covariate-only warnings without more test false alarms. There were only three degraded logistic windows and one degraded forest window.

| Model / warning policy | Test alerts / 28 | True positives | False positives | Misses | Recall | False-alarm rate | Gated coverage | Gated risk |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Logistic combined / TRIAD | 8 | 2 | 6 | 1 | 66.67% | 24.00% | 55.55% | 30.39% |
| Logistic covariate only | 6 | 2 | 4 | 1 | 66.67% | 16.00% | 59.89% | 30.23% |
| Logistic confidence only | 19 | 2 | 17 | 1 | 66.67% | 68.00% | 26.70% | 30.61% |
| Forest combined / TRIAD | 17 | 1 | 16 | 0 | 100.00% | 59.26% | 26.50% | 28.16% |
| Forest covariate only | 6 | 1 | 5 | 0 | 100.00% | 18.52% | 47.59% | 26.06% |
| Forest confidence only | 24 | 1 | 23 | 0 | 100.00% | 85.19% | 10.03% | 25.59% |

Combined warnings added **zero true detections**, plus two/eleven false alarms for logistic/forest. All policies detected the first window of their one degraded episode, giving recorded delay zero windows. This means end-of-window batch detection, not prospective lead time. Logistic still missed one later degraded window. A single episode per model cannot characterize detection delay reliably.

The TRIAD gate withholds otherwise accepted predictions throughout an alerted batch. Its logistic risk change is -0.7972 percentage points (seven-day interval [-2.7794,+0.9694]); forest change is **+2.3168** points ([-0.5370,+5.1529]). Both intervals include zero, with large coverage costs. Fewer incorrect acceptances per all observations can result mechanically from refusing more answers and must not be sold as improved conditional reliability.

The simple combined-warning baseline uses exactly the same decisions and therefore has exactly the same metrics. Metadata does not improve the predictor. Removing distribution or confidence diagnostics yields the two explicit ablations above. Numerical status is not applicable to these models; statistical insufficiency and calendar uncertainty are fixed context. Removing those fixed annotations changes no scores, warnings or decisions. Their possible communication/audit utility was not measured in a user study. **Transparency is the supported contribution; scientific added-value superiority is not.**

## Uncertainty, power and interpretation

One thousand paired circular moving-block replicates resample ordered day units with block lengths 1, 7 (primary) and 14. Within-day observations remain together and longer blocks retain nearby days. Models, acceptance and alert thresholds remain fixed. Circular wrapping and approximate local stationarity are assumptions, not properties established by the data. The one-day analysis is a sensitivity endpoint, not a claim that adjacent days are independent.

Block resampling has established stationary-series foundations, for example H. R.
Künsch, *The jackknife and the bootstrap for general stationary observations*,
Annals of Statistics 17 (1989), 1217–1241 ([author's publication record](https://people.math.ethz.ch/~kuensch/papers/)).
That literature does not certify the stationarity or interval coverage of this dataset.

Logistic selective-risk intervals widen from [28.9760%,33.5928%] at one day to [27.6823%,34.7750%] at fourteen days. Forest widens from [23.6255%,28.1865%] to [22.1358%,29.7340%]. All lengths and ablations are retained in the report. No overlapping evaluation windows are counted as independent observations. Each whole split is one dependent sequence, deliberately disabling the generic independent-cluster bootstrap. Single-window seven/fourteen-day intervals are unavailable with fewer than two blocks. Zero/all error events and missing truth do not produce spurious zero-width risk intervals.

Warning recall/false-alarm intervals are left null: 28 dependent windows, three/one positive windows and one episode per model do not support a defensible independent-trial calculation. Missing outcomes are unknown, never nondegraded. The report includes all event denominators. No p-values, familywise significance or confirmatory power claim is made; two models, many windows and ablations are exploratory, with no favorable result selected.

Prospective planning: even a single independent Bernoulli fraction needs approximately 385 independent units for a worst-case 95% half-width of 0.05. A future paired warning study must recruit independent streams and enough **degraded** episodes, define label availability and operational warning cost, and estimate paired discordance/dependence on development evidence. For illustration only, a paired recall gain of .10 with assumed discordance .30 gives variance .29; a normal approximation with 80% power and two-sided .05 uses about 228 independent positive units. This is not power for the current correlated data, and multiple-model confirmation would need an adjusted design. See the frozen protocol's precision limits.

## Verification and limitations

Rust implements strict manifest validation, existing generic metrics, fixed policies, temporal uncertainty and deterministic report replay. A separate NumPy implementation checks coverage, risk, incorrect acceptance, abstention, full AURC curves, Brier, ECE/reliability bins, thresholds, alert counts/confusion, ablations and all aggregate paired block intervals at relative 1e-11 / absolute 1e-12 tolerance. Rust adversarial tests cover malformed windows/order, unsupported assurance, missing labels, label-independent warnings, degenerate intervals and report tampering. This is independent **code**, not independent **investigators**.

Retraining checks predictions, diagnostics, scaler, coefficients and trees with explicit floating-point tolerances and separately verifies each model-file hash. Dataset/model provenance checks are distinct from mathematical replay, which cannot authenticate supplied attestations. A SHA-256 manifest protects 73 pre-M6 experiment/result artifacts. Linux/Windows CI runs the new benchmark, archived replay, independent calculations, source-data retraining and existing tests. The PR's CI records provide the platform execution status; local checks alone do not imply both platforms passed.

Unresolved limitations: one historical preprocessed dataset, uncertain calendar encoding, possible original imputation leakage, fixed lightweight models, mean-only shift detectors, small degraded-event counts, no unrelated-team replication, no hardware/3D validation, no guarantee under unseen distributions, no calibrated financial forecasting claim, no cost-utility study, and no evidence that fixed validity annotations improve decisions. Computational reproducibility does not remove these limitations.

## Reproduction and NIST preparation

Follow [the benchmark commands and pinned dependencies](../../benchmarks/electricity/README.md). Outputs require new directories. [PNG](../../results/benchmark-electricity/plots/electricity.png) and [PDF](../../results/benchmark-electricity/plots/electricity.pdf) figures show retrospective curves, event reliability and nonoverlapping temporal windows; the captions explicitly qualify chronology and independence. They are measured results, not illustrations of an expected win.

The [NIST readiness assessment](nist-readiness.md) and [draft](nist-submission-draft.yml) include the negative outcome and public contact `ciao_chris@proton.me`. The current official schema and reviewer expectations are checked separately. Schema conformance is not scientific approval. Distinct utility beyond existing risk-coverage/calibration/refusal tools remains unestablished. **Nothing has been submitted to NIST.** A separate human scientific review and explicit submission decision are still required.
