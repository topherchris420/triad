# Validity-qualified selective-risk evaluation

Object: a fixed model's predictions and uncertainty scores on a declared sample.
For loss L, tolerance epsilon and smaller-is-more-confident score s:

$$C(\tau)=P(s\leq\tau),\qquad R(\tau)=P(L(\hat y,y)>\epsilon\mid s\leq\tau).$$

Incorrect-acceptance frequency is the joint frequency of acceptance and excessive
error, with all predictions as denominator. Abstention is one minus coverage.
The score is not assumed to be a probability. Optional `error_probability`
specifically predicts the event L > epsilon, not class membership.

Operational threshold: sort scores from the declared calibration scenario only,
take index ceil(n*c)-1 and accept all ties. c=0 rejects all. This targets empirical
calibration coverage, not guaranteed test coverage or risk. Calibration labels
are not used. Sequence and optional spatial IDs must be disjoint across splits.
This verifies declared IDs, not the honesty of externally supplied provenance.

Retrospective ranking: accept floor(n*c) test rows, sorted by score then unique
record ID. It is descriptive and must never be deployed as a fitted threshold.
The risk-coverage curve accepts complete score tie groups. AURC is the sum of
each group's coverage increment times cumulative risk after accepting that group;
it is a right-endpoint step integral, not a trapezoidal integral or arbitrary
tie-breaking average. It is undefined when labels are missing.

ECE uses declared equal-width probability bins on [0,1], the last inclusive of 1.
It is the count-weighted absolute difference between event frequency and mean
error probability. Brier is mean squared probability error. Aggregate values
are null if any test row lacks a probability or label; bin counts explicitly
describe the available subset. Rare failures and coarse bins can conceal severe
miscalibration. Examine scenario results and event counts, not only ECE.

Whole-sequence and whole-spatial-cluster percentile bootstrap intervals condition
on the fitted model and fixed threshold. Each replicate draws G clusters with
replacement and includes every row of each draw, including repeated clusters.
The statistic is frame-weighted. The two clustering analyses are separate, not
a valid substitute for crossed-cluster inference. Undefined replicates invalidate
the interval. Zero/all-error accepted sets and fewer than two clusters return
null intervals; absence of failures does not prove zero population risk.

Numerical, sampling, distribution and evidence status are distinct. User-supplied
evidence is an attestation, never independently certified by this evaluator.
Physical confidence remains unvalidated. These are exploratory descriptive
metrics even when the code passes every software test.
