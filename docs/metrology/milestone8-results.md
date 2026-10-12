# Milestone 8 results: independent, outcome-grounded test of evidence sufficiency

**Executed October 11, 2026. Vers3Dynamics, PI Christopher Woodyard.**

**Bottom line.** On 5,078 reliability claims about five public datasets, with truth
taken from real held-out outcomes, TRIAD's claim-relative evidence rules did **not**
beat independently designed strong comparators.

- A blind statistical reviewer (S) had lower decision cost: 0.121 versus 0.159,
  Δ = +0.038, 95% interval [+0.008, +0.070].
- A blind REFORMS-style checklist (R) was indistinguishable from TRIAD: 0.160,
  Δ = −0.001 [−0.034, +0.035].
- TRIAD also cost more than never endorsing any claim (0.125).

The pre-registered verdict is **not established**. TRIAD's evidence-sufficiency
method should not be presented to NIST, or anywhere else, as a new contribution
on the basis of current evidence.

The Milestone 7 finding that TRIAD's rules beat standard evidence checks
**replicates with world truth**: Δ = −0.172 [−0.206, −0.135] versus C, and
Δ = −0.050 [−0.082, −0.019] versus C+scope. But that advantage is reproduced by
reviewers written without seeing TRIAD, so it is not distinctive.

## 1. Why this test is independent of Milestone 7

Milestone 7's 64-case suite had one author for the rules, cases and ground truth.
Here:

- **Truth comes from the world.** Every claim concerns a finite, real population
  of predictions (an evaluation year, a month, a sensor batch, a week). Its truth
  is computed mechanically from held-out outcomes, sealed in `truth.json`, and never
  seen by any classifier.
- **The data are new to TRIAD.** Beijing multi-site air quality, Minneapolis I-94
  traffic, Sceaux household power, gas-sensor array drift and Portuguese bank
  marketing are all CC BY 4.0 from the UCI repository. TRIAD's rules
  (`src/evidence.rs`) are byte-identical to Milestone 7, which a hash-manifest
  test enforces.
- **Cases are generated, not chosen.** A generator fixed in advance produced
  every case from pre-registered sampling designs and claim templates. All cases
  are scored.
- **The strong comparators were written blind.** A separately briefed agent wrote
  them from [`comparator-brief.md`](../../benchmarks/evidence-outcomes/comparator-brief.md)
  alone, with no access to TRIAD's rules, documents or results
  ([provenance](../../benchmarks/evidence-outcomes/comparators/README.md)).

| Commit | Content | Before |
|---|---|---|
| `9df849e` | [Protocol](../../benchmarks/evidence-outcomes/protocol.json), comparator brief, hash manifest of all Milestone 7 evidence | any model, case or comparator |
| `bbde23a` | Generator, Rust `evidence-batch`, scorer, verifier, [implementation notes](../../benchmarks/evidence-outcomes/implementation-notes.md), [data audit](../../benchmarks/evidence-outcomes/data-audit.json) (counts only) | any model fitted on real data |
| `152b012` | Blind comparators R and S, byte-identical to delivery | any real case |
| `1d065ac` | The single run's outputs | — |

**Limits of independence.** The claim templates, sampling designs and encoding rules
were written by the project that wrote TRIAD's rules. The comparator author is an
AI agent of the same model family, and its isolation was instruction-based and
self-reported. There has been no human adjudication, no unrelated-team
replication and no peer review.

## 2. Design

Each dataset has a frozen simple model (ridge on `log1p` for the regressions,
logistic for the classifications), trained on an early period. A **material error**
is a misclassification, or a regression error above max(0.25|y|, 0.25 × training
median). Claims are made at 95% confidence about the later *evaluation* and *future*
periods.

| Family | What is claimed | Evidence | Truth | Claims |
|---|---|---|---|---:|
| A: audit sample | error rate over the 12-month evaluation period ≤ b | a random sample (n = 100/400/1600 units), whole random clusters (4/16/64 days or blocks) or a contiguous block. Each is reported three ways: raw outcomes, an i.i.d. interval, or a design-appropriate interval | the period's actual rate | 4,320 |
| B: extrapolation | error rate in a later month or batch ≤ b | all outcomes of a 3-month window (1 batch for gas) plus a label-free shift statistic | that stratum's actual rate | 342 |
| C: conformal coverage | 90% split-conformal intervals cover ≥ 90% (judged at ≥ 88%) in a target | calibration-set description plus shift statistic. Targets: the random other half of the month, or the month +1, +3 or +6 | realised coverage | 144 |
| D: operational | the coming week's error rate ≤ r_dev + 0.02 or + 0.05 | development-period calibration, monitoring range, completeness, last week's outcomes | the coming week's actual rate | 272 |

Material-error rates by period (train → development → evaluation → future) were:

| Dataset | Train | Development | Evaluation | Future |
|---|---:|---:|---:|---:|
| Air quality | 10.9% | 9.1% | 8.4% | 8.0% |
| Traffic | 1.3% | 2.2% | 1.4% | 1.6% |
| Household power | 47.3% | 48.3% | 43.7% | 44.4% |
| Gas drift | 0.3% | 14.2% | 26.2% | 47.4% |
| Bank marketing | 4.8% | 12.8% | 39.1% | 52.1% |

Gas drift and bank marketing degrade sharply; the others are roughly stable.

**Primary endpoint** (pre-registered): family-balanced decision cost. Endorsing a
false claim costs 1 and withholding endorsement from a true claim costs 0.25, at
equal weight for true and false claims within each family, averaged over
families. Never endorsing therefore costs exactly 0.125. Intervals come from a
stratified bootstrap over resampling clusters (2,000 replicates).

## 3. Primary result

| Method | Cost | 95% interval | Mean FSR | Mean TSR |
|---|---:|---|---:|---:|
| **S**, blind statistician | **0.121** | [0.111, 0.134] | 0.024 | 0.131 |
| never endorse | 0.125 | — | 0 | 0 |
| **TRIAD** | 0.159 | [0.128, 0.193] | 0.120 | 0.209 |
| **R**, blind REFORMS checklist | 0.160 | [0.135, 0.189] | 0.130 | 0.239 |
| TRIAD core (scope, dependence and statistical rules only) | 0.204 | [0.180, 0.224] | 0.252 | 0.377 |
| C+scope, Milestone 7 baseline | 0.209 | [0.185, 0.230] | 0.262 | 0.378 |
| C, standard checks | 0.331 | [0.308, 0.349] | 0.610 | 0.793 |

FSR is the share of false claims endorsed; TSR is the share of true claims endorsed.

| TRIAD minus | Δ cost | 95% interval | One-sided 97.5% upper bound | Superior? | Datasets with Δ < 0 |
|---|---:|---|---:|---|---:|
| S | +0.038 | [+0.008, +0.070] | +0.070 | no (**inferior**) | 1 of 5 |
| R | −0.001 | [−0.034, +0.035] | +0.035 | no | 3 of 5 |
| never endorse | +0.034 | [+0.003, +0.068] | +0.068 | no | 2 of 5 |

Every pre-registered condition failed: superiority to R, superiority to S, mean
FSR ≤ 0.05 (it was 0.120), beating never-endorse, and consistency across datasets.
**Verdict: not established.** Against the better strong comparator (S), TRIAD is
**inferior**.

## 4. Where TRIAD wins and where it loses

| Family | TRIAD FSR / TSR | R FSR / TSR | S FSR / TSR | C FSR / TSR |
|---|---|---|---|---|
| A, 627 false / 3,693 true | 0.006 / 0.224 | 0.021 / 0.283 | 0.018 / 0.298 | 0.311 / 0.616 |
| B, 107 / 235 | 0 / 0 | 0 / 0 | 0 / 0 | 0.234 / 0.621 |
| C, 38 / 106 | 0.105 / 0.264 | 0.395 / 0.547 | 0.079 / 0.142 | 1.000 / 1.000 |
| D, 19 / 253 (fewer than 20 false: flagged) | 0.368 / 0.348 | 0.105 / 0.126 | 0 / 0.083 | 0.895 / 0.933 |

- **Extrapolation (B).** Refusing to extrapolate was right at this cost ratio. C
  endorsed 23% of false extrapolations (25 of 107), most of them bank-marketing
  months after the 2009–2010 shift (14). But R and S, written blind, refuse too: the
  scope rule is ordinary good practice, not a TRIAD distinction.
- **Audit samples (A).** All three serious reviewers are almost never wrong when
  they endorse. TRIAD endorses fewer true claims (22% versus S's 30%), because it
  fails closed on raw dependent outcomes. S estimates the design effect from the
  outcomes themselves, by batch means, and uses it. S endorses and TRIAD declines on 278
  family-A claims, 271 of them true. Every one is blocked by TRIAD's statistical
  rule, alone or together with the dependence rule. In family A, TRIAD never
  endorses a claim that S declines.
- **Operational next-week claims (D): TRIAD's clearest weakness.** TRIAD's
  operational rules contradict a claim only when *last* week's lower bound
  already exceeds the bound. Otherwise they endorse it, with no forecast of
  *next* week. TRIAD endorsed 7 of the 19 false next-week claims (all on air
  quality and household power), which made it worse than never-endorse in this
  family. S, using a beta-binomial predictive bound, endorsed none.
- **Conformal coverage (C).** TRIAD endorsed only random-split claims, where
  exchangeability holds by construction (S also endorsed a few of these). Four of
  TRIAD's were false, two each on traffic and power, with realised coverage of
  87.6–87.9% on halves of about 350 units. That is just below the 88% cut-off: the
  ordinary sampling variability of a finite random half, which any reviewer
  trusting conformal theory accepts. R
  endorsed next-month coverage whenever the shift statistic was below its
  threshold, and 15 of 38 false claims got through.

## 5. Secondary analyses (pre-registered)

- **Replication of Milestone 7 with world truth.** TRIAD beats C
  (Δ = −0.172 [−0.206, −0.135]) and C+scope (Δ = −0.050 [−0.082, −0.019]). Milestone 7's
  direction holds outside an author-built suite, but it does not survive comparison
  with strong reviewers.
- **Distinctness (F4).** Support decisions agree with C+scope on 92% of claims,
  with R on 93% and with S on 93%. Restricting TRIAD to its scope, dependence and
  statistical families raises its cost to 0.204: the calibration-applicability
  and representativeness rules matter in families C and D.
- **Ablations (cost with one rule family off; full TRIAD is 0.159).**
  - Statistical: 0.181. Calibration applicability: 0.179. Scope: 0.173.
    Dependence: 0.163. These rules help.
  - Representativeness: 0.158. Completeness: 0.155. Removing these slightly
    *lowers* the cost.
  - Integrity, authority, leakage, attestation, limitations, universality and
    applicability: unchanged, because they are not exercised by these claim
    types.
- **Sensitivity.** At c = 0.5: S 0.229, never 0.250, R 0.255, TRIAD 0.258. At
  c = 0.1 never-endorse is cheapest: never 0.050, S 0.056, TRIAD 0.100, R 0.103.
  Coverage tolerance 0.01: S 0.119, TRIAD 0.157, R 0.165. Tolerance 0.03: S 0.110,
  TRIAD 0.145, R 0.145. TRIAD never beats S or never-endorse.
  - On the empirical-prevalence pooled cost, which is dominated by family A
    claims (true 85%), every reviewer beats never-endorse: S 0.158, C+scope 0.158,
    R 0.161, TRIAD 0.168.
  - Under that weighting C is lowest at 0.129, because it endorses most of the
    many true audit claims.
- **Precision of "contradicted".** TRIAD contradicted 7 claims, all operational
  (family D), and only 2 were false. Rule R4 contradicts when *last* week's
  dependence-adjusted lower bound exceeds the bound. In the 5 true cases the
  coming week recovered. This is the same missing forecast that lets false
  next-week claims through.

## 6. A flaw in the pre-registered validity criterion (post hoc, clearly labelled)

The protocol required "mean FSR ≤ 0.05", the share of *false* claims endorsed.
After the run I found that this is **not a property a valid 95% method
guarantees** when the claimed bound is derived from the same sample.

- With the template b = upper Clopper–Pearson bound, every exact method endorses
  every such claim. The false ones are exactly the bound's 5% miss cases, so all
  of them are endorsed.
- What validity guarantees is a low rate of endorsed false claims among *all*
  claims, P(endorsed and false).

Measured that way (not pre-registered):

| Family | TRIAD | R | S | C | C+scope |
|---|---:|---:|---:|---:|---:|
| A | 0.001 | 0.003 | 0.003 | 0.045 | 0.007 |
| B | 0 | 0 | 0 | 0.073 | 0 |
| C | 0.028 | 0.104 | 0.021 | 0.264 | 0.028 |
| D | 0.026 | 0.007 | 0 | 0.062 | 0.062 |

TRIAD, S and C+scope are well within 5% everywhere. The verdict does not depend on
this flaw. TRIAD also failed every cost-based condition, which is unaffected. Future
protocols should use the unconditional rate.

## 7. Verification and reproduction

- [`scripts/verify-milestone8.py`](../../scripts/verify-milestone8.py) passes
  109,685 checks without calling Rust or importing the generator or scorer:
  - every TRIAD, C, C+scope, core and ablation classification is recomputed by
    the separately written Python re-implementation of the Milestone 7 rules;
  - truth values are checked against period, stratum and week counts;
  - claim templates are recomputed with SciPy;
  - every endpoint, bootstrap bound and the decision are recomputed.
- A second generation from the downloaded archives was **byte-identical**.
- Linux and Windows CI re-run the batch, comparators and scorer, compare them with
  the archive, regenerate every case from the public archives and run the
  verifier.

```sh
python -m pip install -r benchmarks/evidence-outcomes/requirements.txt
python scripts/m8-generate.py --cache work/m8-cache --output work/m8
cargo run --release --locked -- metrology evidence-batch --input work/m8/cases.json --output work/m8/batch
cp work/m8/batch/classifications.json work/m8/
python benchmarks/evidence-outcomes/comparators/m8_comparators.py --input work/m8/cases.json --output work/m8/comparators.json
python scripts/m8-score.py --truth work/m8/truth.json --triad work/m8/classifications.json --comparators work/m8/comparators.json --output work/m8/score
python scripts/verify-milestone8.py --run work/m8
```

Archived outputs are in [`results/evidence-outcomes-v8/`](../../results/evidence-outcomes-v8/):
`cases.json.gz`, `truth.json`, `units.json`, `classifications.json`,
`comparators.json` and `score/report.json`.

## 8. What this means

1. **For NIST.** Submit only the established selective-risk metric, with TRIAD as
   one implementation ([package](nist-submission/README.md)). Do not propose the
   evidence-sufficiency method: an independent, outcome-grounded test with strong
   comparators found it no better than a blind checklist and worse than a blind
   statistician.
2. **For TRIAD's research.** These results point to specific, testable repairs:
   - a predictive bound for operational claims;
   - design-effect estimation from raw ordered outcomes, instead of failing closed;
   - possibly dropping the completeness and representativeness gates, which
     slightly hurt here.

   Any repaired rule set would need a **new** frozen test on new data. Re-scoring
   this run after changing rules would be tuning to the test.
3. **Still untested.** Leakage, provenance, attestation and numerical-adequacy
   rules (outside the claim families that can be checked against outcomes);
   human-adjudicated published claims; reviewer-utility studies.
