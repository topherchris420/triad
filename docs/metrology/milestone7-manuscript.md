---
title: "Does Claim-Relative Evidence Qualification Improve Predictive Reliability Decisions? A Pre-specified Evaluation with Negative Predictive Results"
author: "Christopher Woodyard (Vers3Dynamics)"
date: "October 11, 2026 — preprint draft, not peer reviewed"
---

# Does Claim-Relative Evidence Qualification Improve Predictive Reliability Decisions? A Pre-specified Evaluation with Negative Predictive Results

Christopher Woodyard, Vers3Dynamics. Draft of October 11, 2026. **Not peer reviewed.**
No independent replication has been performed. Not a NIST submission.

*Title note.* The working title, "TRIAD: Evidence-Sufficiency Evaluation for
Predictive Reliability Under Distribution Shift", was replaced because the results
do not support an evidence-sufficiency contribution to predictive reliability.

## Abstract

**Background.** Reliability reports for predictive systems often state claims, such
as "the error rate is below 10%", that the underlying evidence supports only for
narrower populations, conditions or sampling assumptions. We asked whether making
claims and evidence explicit, typed and rule-checked can (i) improve operational
trust decisions under natural temporal change and (ii) identify unsupported
reliability claims better than standard evidence checks.

**Methods.** We implemented TRIAD, a Rust evaluator that combines established
selective-risk, calibration and conformal measurements with a fail-closed,
claim-relative evidence classifier (supported within scope / insufficient /
contradicted / not applicable). In Experiment A, ridge and gradient-boosting models
made one-hour-ahead forecasts of hourly UCI Bike Sharing demand. Each day, policies
decided prospectively whether to trust the models. We compared TRIAD's
evidence-qualified decisions with covariate, confidence and delayed-label
monitoring at matched calibration alert budgets, using a pre-specified cost endpoint
and paired moving-block bootstrap intervals. In Experiment B, TRIAD and a standard
evidence-check baseline classified 64 claims with pre-specified ground truth. The
protocols, claim suite and models were committed to Git before any outcome was
computed.

**Results.** TRIAD's day decisions cost more than standard monitoring for both
models: +0.0049 per hour (Bonferroni 97.5% interval +0.0006 to +0.0105) and +0.0062
(+0.0014 to +0.0122). Both were below the pre-specified practical threshold of 0.01,
and no day-level gate beat no gating at the primary abstention cost. In the claim
suite, TRIAD detected 43 of 47 unsupported claims versus 20 for standard checks, with
no false rejections. It failed every case with deliberately misleading evidence. The
rules, cases and ground truth shared one author.

**Conclusions.** Explicit evidence qualification did not improve predictive
reliability decisions; this replicates an earlier negative result on a second
dataset. Its audit advantage was shown only on an author-constructed suite and
reduces to explicit rules that standard checklists could adopt. Further independent
validation is required before claiming a distinct methodological contribution.

## 1. Introduction

A system that refuses to answer is not thereby more trustworthy, and a system that
reports more warnings is not thereby more reliable. TRIAD began as a study of when a
sparse capacitive sensor should refuse to report a position. Its simulated physics
studies (Milestones 2–5) found that optimizer convergence is not measurement
confidence, and that its numerical model failed its own readiness gates. Those
results are **simulated** and **separate** from the real-world AI benchmarks
reported here; nothing in this paper depends on them.

Milestone 6 moved to real data (OpenML Electricity) and found that adding validity
annotations and combined warnings to selective prediction produced no additional
detections and more false alarms. This paper asks a narrower and more falsifiable
question:

- **RQ1 (predictive reliability).** Do claim-relative evidence assessments, made
  prospectively and with the same information as established monitoring, lead to
  better trust decisions?
- **RQ2 (claim validity).** Do explicit claim-relative rules identify unsupported
  reliability claims more often than standard evidence checks, without rejecting
  valid claims more often?

**Pre-specified hypotheses.** H1: for both models, evidence-qualified day gating
reduces mean decision cost relative to standard monitoring at a matched
calibration budget by at least 0.01 per hour, with Bonferroni-adjusted confidence.
H2: on the frozen suite, TRIAD's unsupported-claim detection rate exceeds that of
standard checks by at least 0.20, without raising the false-rejection rate by more
than 0.05.

## 2. Related work

Selective prediction and the error–reject trade-off are established (Chow 1970;
El-Yaniv & Wiener 2010; Geifman & El-Yaniv 2017), as are probability calibration
diagnostics (Brier 1950; Guo et al. 2017). Conformal prediction gives distribution-free
intervals under exchangeability (Vovk et al. 2005; Papadopoulos et al. 2002; Lei et
al. 2018; Angelopoulos & Bates 2023), with extensions beyond exchangeability, for
online adaptation and for risk control (Barber et al. 2023; Gibbs & Candès 2021;
Angelopoulos et al. 2024). Dataset-shift detection with univariate two-sample tests
performs well in empirical comparisons (Quiñonero-Candela et al. 2009; Lipton et al.
2018; Rabanser et al. 2019). Performance-based drift detection and sequential
monitoring date from CUSUM (Page 1954) to error-rate and windowing detectors (Gama
et al. 2004; Bifet & Gavaldà 2007; Gama et al. 2014). Dependent-data inference uses
block bootstraps (Künsch 1989; Politis & Romano 1994). Leakage is formalised by
Kaufman et al. (2012) and catalogued for ML-based science by Kapoor & Narayanan
(2023). Reporting checklists include model cards, datasheets, REFORMS and TRIPOD+AI
(Mitchell et al. 2019; Gebru et al. 2021; Kapoor et al. 2024; Collins et al. 2024).
Structured claims–arguments–evidence reasoning is the basis of assurance cases
(Kelly & Weaver 2004). TRIAD's evidence classifier is a small, executable instance of
that idea for predictive-reliability claims; we do not claim it is novel in kind.

## 3. Methods

### 3.1 Selective risk and calibration

For loss $L$, tolerance $\varepsilon$ and score $s$ (smaller is safer),
$C(t)=P(s\le t)$ and $R(t)=P(L>\varepsilon\mid s\le t)$. The operational threshold is the
$\lceil nc\rceil$-th smallest calibration score (ties accepted, $c=0.8$). For
regression, the material-error event $|\hat y-y|>\max(a, r y)$ is passed to the
generic evaluator as the normalised error $u=(\hat y-y)/\max(a,ry)$ with
tolerance 1. Normalised split-conformal intervals use
$q=\big(\lceil (n+1)(1-\alpha)\rceil\big)$-th smallest $|y-\hat y|/s$ on calibration data.

### 3.2 Claim-relative evidence classification

A claim has a type, a population, a condition set $S_c$ and links to evidence.
Evidence items are *computed* (TRIAD recomputes them), *supplied* (structured data
that TRIAD checks for consistency but cannot verify) or *attested* (bare assertions).
Rules return support, insufficient, contradict or not applicable for one of twelve
dimensions. The classification is not applicable if any rule says so. Otherwise it
is insufficient if a linked integrity check failed, contradicted if any rule
contradicts, insufficient if any rule is insufficient **or no rule supports**, and
otherwise supported within scope. The axioms are: unknown evidence never passes;
attestations are never relied upon; a hash establishes byte consistency only; claimed
conditions must have been evaluated; and dependent units require a dependence-aware
interval or a design effect. Inferential bounds use one-sided Clopper–Pearson limits
(Clopper & Pearson 1934), with effective counts $n/D$ and $k/D$ under a design effect
$D$. Lag-1 dependence is significant when $|\rho|>1.96/\sqrt n$ (Bartlett 1946).
Section 4 of the [method specification](evidence-sufficiency-method.md) gives all ten
claim types and their rules. No scalar evidence score is produced.

The baseline (C, *standard evidence checks*) receives the same input and performs
schema validation, missing-evidence warnings, hash checks and metric recomputation
with supplied or i.i.d. Clopper–Pearson intervals, and it accepts checklist
attestations. It does not reason about scope, dependence, calibration applicability,
authority, limitations, universality or not-applicability. A pre-declared variant
(C+scope) adds the scope rule.

### 3.3 Prospective day-level trust decisions

At the start of each day $d$, a policy chooses to trust or withhold all of that day's
otherwise-accepted hourly predictions. It may use data through day $d-1$ and the
calendar of $d$. Labels arrive one hour after each target, so the outcomes of day
$d-1$ are available. The cost per eligible hour is 1 for an accepted material error
and $c$ for any withheld prediction; the primary value is $c=0.25$.

- *cov*: maximum two-sample KS statistic over the five demand-history features, day
  $d-1$ versus development (Massey 1951; Rabanser et al. 2019).
- *conf*: mean error probability on day $d-1$.
- *perf*: accepted material-error rate on day $d-1$.
- *std*: maximum of normalised *cov* and *perf*, plus a completeness warning.
- *TRIAD*: an `operational_risk` claim, "accepted risk on $d$ is at most
  $r_{cal}+0.05$". Rule R1 requires at least five calibration days of $d$'s day
  type; R2 requires the day $d-1$ KS statistic to be within its calibration
  maximum; R3 requires at least 22 eligible hours on day $d-1$; R4 marks the claim
  contradicted when the design-effect-adjusted Clopper–Pearson lower bound for day
  $d-1$ exceeds the bound. Any non-supported claim withholds the day.

Comparators were matched to TRIAD's calibration alert count (primary), set at their
own 80th calibration percentile (native), and ranked top-*k* on test days to match
TRIAD's test alert count (descriptive).

## 4. Data and models

UCI Bike Sharing (Fanaee-T 2013; Fanaee-T & Gama 2014; CC BY 4.0) has 17,379 hourly
rows for 2011–2012 in Washington, D.C. Timestamps are New York wall-clock labels.
Mapped to elapsed hours, 165 hours are missing in 75 runs, including 36 hours during
Hurricane Sandy. The 02:00 label is absent on spring-forward days, and fall-back days
carry 24 labels for 25 hours. Lags are elapsed-hour differences; a target is
eligible only if it and all five lags exist, and nothing is imputed. `casual` and
`registered` sum exactly to the target and are excluded, as is all observed weather.
The partitions are training 2011, development January–April 2012, calibration
May–August, and test September–December, giving 2,806 eligible test hours. The
models, both fitted on `log1p` demand with identical partitions and information, are
ridge regression and histogram gradient boosting (Pedregosa et al. 2011). Per model,
an error-scale model and a logistic error-probability model were fitted on
development data only.

## 5. Experimental protocol and statistics

The protocol (task, tolerance, models, policies, endpoints, thresholds, bootstrap
design, multiplicity) and the 64-case claim suite with ground truth were committed
before fitting. Models and predictions were committed before any metric was
computed. One pre-results amendment added a descriptive outcome-free comparison.
Primary inference uses a paired circular moving-block bootstrap of test days
(Künsch 1989), with 7-day blocks (sensitivity 1 and 14), 2,000 replicates, the same
resampled days for every comparison, and Bonferroni adjustment over the two models.
An advantage requires $\Delta\le-0.01$ and an upper limit below 0; a disadvantage
requires $\Delta\ge0.01$ and a lower limit above 0. For Experiment B, statistics are
suite-internal; the cases are not a random sample of real claims.

## 6. Results

### 6.1 Conventional evaluation

Ridge: test MAE 32.8 bikes, material-error rate 9.7%, selective coverage 82.4% at
risk 9.9%, conformal 90% coverage 88.0%. Gradient boosting: MAE 51.2, rate 21.0%,
coverage 81.9% at risk 14.9%, conformal 90.0%. Both beat persistence and seasonal
naive forecasts (MAE 69.6–83.1). Both under-predicted the 2012 growth, and the tree
ensemble could not extrapolate (September bias −56.7). Monthly conformal coverage
for the tree model rose from 82.8% to 96.7% as autumn demand fell below summer
levels.

### 6.2 Experiment A

| | Ridge | Gradient boosting |
|---|---|---|
| Δ cost, TRIAD − std (matched) | +0.0049 [+0.0006, +0.0105] | +0.0062 [+0.0014, +0.0122] |
| Decision | insufficient to establish an advantage | insufficient to establish an advantage |
| TRIAD − no gate | +0.0170 [+0.0075, +0.0290] | +0.0139 [+0.0044, +0.0271] |
| TRIAD alert days: calibration / test | 3 / 23 | 2 / 21 |
| Degraded days caught, TRIAD / std (of 33 / 27) | 7 / 4 | 4 / 1 |
| Risk of withheld vs retained accepted predictions (TRIAD) | 13.0% vs 9.3% | 14.2% vs 15.0% |

H1 is not supported. No gating policy beat no gating at $c=0.25$; gating helped
only at $c=0.10$, where TRIAD did not beat a native covariate monitor. TRIAD's
alert budget grew from about 2% of calibration days to 18% of test days, because
autumn demand left the summer calibration support (R2, 14 days) and because of data
gaps (R3, 10 days). At equal test alert counts it was indistinguishable from
standard monitoring (Δ = −0.0025 [−0.0108, +0.0061] and +0.0004 [−0.0063, +0.0065]).
Removing any single TRIAD rule lowered the cost or left it unchanged. With the strictest tolerance, the
disadvantage for ridge met the practical threshold (+0.0158 [+0.0039, +0.0321]).

### 6.3 Experiment B

| Method | Unsupported detected | False rejections | Four-class accuracy |
|---|---:|---:|---:|
| TRIAD | 43/47 | 0/16 | 60/64 |
| Standard checks | 20/47 | 0/16 | 34/64 |
| Standard checks + scope | 26/47 | 0/16 | 40/64 |

H2 is met on this suite (difference 0.49). On the 13 repository-record cases, TRIAD
was correct on all; standard checks missed a calendar-chronology claim and a
matched-coverage claim. Every discordant case traces to one simple rule family
(dependence, scope, leakage mechanics and others), and no single family accounts for
more than five. TRIAD failed all four misleading-input cases: it verifies
consistency, not truth.

## 7. Negative findings

1. Evidence-qualified gating did not improve, and slightly worsened, decision cost
   (both models; disadvantage met the practical threshold under one sensitivity
   tolerance).
2. No day-level gating policy improved on per-hour selective prediction at the
   primary cost.
3. Detection of degraded days was poor for every method (best recall 33–41% at
   false-alarm rates of 24–28%).
4. Conformal coverage was unstable across months under seasonal change.
5. The claim-validity advantage depends on a suite written by the method's author.
6. Milestone 6 (Electricity) found the same absence of predictive added value.

## 8. Threats to validity

*Construct:* the tolerance and abstention cost are conventions, and the degraded-day
definition is relative to calibration risk. *Internal:* the same author designed the
policies, the rules and the claim suite, and only Git timestamps evidence
prespecification. *Statistical:* hours and days are dependent; block bootstraps
assume local stationarity; detection metrics come from a few dependent episodes.
*External:* one city, one autumn and two simple models; the claim suite is mostly
constructed. *Implementation:* verification is by independent code from the same
team, not independent investigators.

## 9. Limitations

TRIAD does not certify reliability, does not detect concept drift, cannot anticipate
events absent from its inputs (for example hurricanes, since weather was excluded),
and cannot detect false supplied evidence. Evidence classifications are rule-based
and not calibrated probabilities. The physics-side work remains simulated, with
failed numerical readiness gates.

## 10. Reproducibility

Code, data manifest, protocols, archived reports and figures:
<https://github.com/topherchris420/triad>. See the
[reproduction package](milestone7-reproduction.md) for the commands, versions,
hashes, tolerances and platform notes. An independent NumPy/SciPy verifier
recomputes every reported number and classification (3,027 checks). Retraining
from the public archive reproduces the frozen predictions within stated tolerances.

## 11. Conclusion

Within the tested conditions, explicit claim-relative evidence qualification did not
improve predictive reliability decisions over standard monitoring, and no
day-level gate improved on per-hour selective prediction. Its value, if any, is in
auditing claims. On an author-constructed suite it identified unsupported claims
that standard checks missed, but that advantage reduces to explicit rules a
checklist could encode, and it fails when evidence is misrepresented. The
contribution supported by the evidence is methodological and educational, not
predictive. Establishing a distinct contribution requires an independently
constructed claim suite, a study of decision or audit outcomes, and independent
replication.

## References

C. K. Chow (1970), IEEE Trans. Inf. Theory 16(1):41–46, doi:10.1109/TIT.1970.1054406 ·
R. El-Yaniv & Y. Wiener (2010), JMLR 11:1605–1641 ·
Y. Geifman & R. El-Yaniv (2017), NeurIPS 30 ·
G. W. Brier (1950), Mon. Weather Rev. 78(1):1–3 ·
C. Guo et al. (2017), ICML, PMLR 70:1321–1330 ·
V. Vovk, A. Gammerman & G. Shafer (2005), Springer, doi:10.1007/b106715 ·
H. Papadopoulos et al. (2002), ECML, doi:10.1007/3-540-36755-1_29 ·
J. Lei et al. (2018), JASA 113(523):1094–1111, doi:10.1080/01621459.2017.1307116 ·
A. N. Angelopoulos & S. Bates (2023), FnT ML 16(4), doi:10.1561/2200000101 ·
R. F. Barber et al. (2023), Ann. Statist. 51(2):816–845, doi:10.1214/23-AOS2276 ·
I. Gibbs & E. Candès (2021), NeurIPS 34, arXiv:2106.00170 ·
A. N. Angelopoulos et al. (2024), ICLR, arXiv:2208.02814 ·
J. Quiñonero-Candela et al. (2009), MIT Press, doi:10.7551/mitpress/9780262170055.001.0001 ·
Z. C. Lipton, Y.-X. Wang & A. J. Smola (2018), ICML, PMLR 80:3122–3130 ·
S. Rabanser, S. Günnemann & Z. C. Lipton (2019), NeurIPS 32, arXiv:1810.11953 ·
F. J. Massey (1951), JASA 46(253):68–78, doi:10.1080/01621459.1951.10500769 ·
E. S. Page (1954), Biometrika 41(1–2):100–115, doi:10.1093/biomet/41.1-2.100 ·
J. Gama et al. (2004), SBIA, doi:10.1007/978-3-540-28645-5_29 ·
A. Bifet & R. Gavaldà (2007), SDM, doi:10.1137/1.9781611972771.42 ·
J. Gama et al. (2014), ACM Comput. Surv. 46(4):44, doi:10.1145/2523813 ·
C. J. Clopper & E. S. Pearson (1934), Biometrika 26(4):404–413, doi:10.1093/biomet/26.4.404 ·
M. S. Bartlett (1946), Suppl. JRSS 8(1):27–41, doi:10.2307/2983611 ·
H. R. Künsch (1989), Ann. Statist. 17(3):1217–1241, doi:10.1214/aos/1176347265 ·
D. N. Politis & J. P. Romano (1994), JASA 89(428):1303–1313, doi:10.1080/01621459.1994.10476870 ·
S. Kaufman et al. (2012), ACM TKDD 6(4):15, doi:10.1145/2382577.2382579 ·
S. Kapoor & A. Narayanan (2023), Patterns 4(9):100804, doi:10.1016/j.patter.2023.100804 ·
S. Kapoor et al. (2024), Sci. Adv. 10(18):eadk3452, doi:10.1126/sciadv.adk3452 ·
G. S. Collins et al. (2024), BMJ 385:e078378, doi:10.1136/bmj-2023-078378 ·
M. Mitchell et al. (2019), FAT*, doi:10.1145/3287560.3287596 ·
T. Gebru et al. (2021), CACM 64(12):86–92, doi:10.1145/3458723 ·
T. Kelly & R. Weaver (2004), DSN Workshop on Assurance Cases ·
F. Pedregosa et al. (2011), JMLR 12:2825–2830 ·
H. Fanaee-T (2013), UCI ML Repository, doi:10.24432/C5W894 ·
H. Fanaee-T & J. Gama (2014), Prog. Artif. Intell., doi:10.1007/s13748-013-0040-3.

Complete citations: [`milestone7-references.json`](milestone7-references.json).
