# Claim-relative evidence sufficiency (research stage)

Status: **method specification frozen before implementation and before the
validation suite was run** (Milestone 7). It is a research-stage rule system, not
a validated assurance standard. Vers3Dynamics, PI Christopher Woodyard.

## 1. Purpose and boundary

A reliability *claim* is a typed statement about a metric, population, operating
condition or property of a predictive system. *Evidence* is a set of typed records.
The method decides, by explicit rules, whether the linked evidence supports the
claim **at the claim's stated scope**. It does not decide whether the claim is
true in the world, and it does not produce a confidence percentage or an evidence
score: no such scalar has been theoretically justified or validated.

The same evidence can support one claim and fail another. "Model A achieved 80%
calibration coverage on the 2012 test period" can be supported by recomputable
counts; "Model A is reliable under all future distribution shifts" cannot be
supported by any finite evaluation.

This framework is conceptually an instance of the claims–arguments–evidence
structure used in assurance cases (e.g. Goal Structuring Notation; Kelly and
Weaver 2004). Its specific content is a small set of machine-executable rules for
predictive-reliability claims. Novelty relative to assurance-case practice is
**not** claimed.

## 2. Formal definitions

Let a claim be `c = (type, scope, parameters, L)` where `scope` is a population
identifier and a finite set of conditions `S_c`, and `L` is the set of linked
evidence identifiers. Only linked evidence is considered. Each evidence item
`e` has a kind and a **basis**:

- `computed`: TRIAD itself computed the fact during the assessment (a file or
  embedded-content SHA-256, counts from supplied outcome records, quantities
  computed inside a TRIAD benchmark run);
- `supplied`: structured data supplied by the user to which TRIAD applies a rule
  (counts, intervals, feature-timing tables, partition dates, diagnostics). TRIAD
  checks internal consistency but cannot verify that supplied data are true;
- `attested`: a bare assertion of a conclusion (for example "no leakage: yes").

**Rule.** A rule `r` maps `(c, E_L)` to one of `support`, `insufficient(d)`,
`contradict(d)` or `not_applicable`, where `d` is an evidence dimension:
`provenance`, `integrity`, `numerical_adequacy`, `statistical_sample`,
`calibration_applicability`, `distribution_representativeness`,
`temporal_dependence`, `missing_evidence`, `known_limitation`, `population_scope`,
`leakage`, `numerical_validity` (a stated number is not what its evidence computes).

**Classification.** For claim `c` with rule outcomes `O`:

1. `Not applicable` if any rule returns `not_applicable` (the claimed property does
   not exist for the described system).
2. Otherwise, if any linked integrity-bearing evidence failed its hash check and
   `c` is not itself a provenance claim, `Insufficient evidence` (integrity): the
   evidence cannot be relied upon, in either direction.
3. Otherwise `Contradicted by available evidence` if any rule returns `contradict`.
4. Otherwise `Insufficient evidence` if any rule returns `insufficient`.
5. Otherwise `Supported within scope`.

**Fail-closed axioms.**

- A1 (unknown never passes): a requirement whose evidence is absent, null,
  `unknown`, unparseable or only attested is `insufficient`.
- A2 (attestations are reported, never relied upon): no rule is satisfied by an
  attestation alone.
- A3 (hash semantics): a matching SHA-256 establishes byte consistency with the
  recorded value and nothing else. It supports provenance only when the recorded
  value comes from an authority at least as strong as the claim requires
  (`external_source` > `repository_record` > `self_reported`).
- A4 (scope): every condition named by the claim must be among the conditions the
  evidence evaluated (`S_c ⊆ S_e`); otherwise the claim extrapolates.
- A5 (dependence): inferential bounds from dependent units require a
  dependence-aware interval or an explicit design effect; `unknown` dependence is
  insufficient.

## 3. Statistical primitives

For `k` events in `n` units, the one-sided level-`γ` Clopper–Pearson (1934) bounds
are `U = B⁻¹(γ; k+1, n−k)` (with `U=1` when `k=n`) and `L = B⁻¹(1−γ; k, n−k+1)`
(with `L=0` when `k=0`), where `B⁻¹` is the inverse regularised incomplete beta
function. For zero events `U = 1 − (1−γ)^{1/n}`, the exact form behind the
"rule of three". With a design effect `D ≥ 1` the counts become `n/D` and `k/D`
(effective sample size; non-integer beta parameters are used directly). A
supplied interval is used instead of Clopper–Pearson only if its method is
dependence-aware when the data are dependent, and its nominal level is at least
`γ`; a two-sided level-`γ` interval is conservative for one-sided use.

A lag-1 autocorrelation `ρ` from `n` ordered units is *significant* when
`|ρ| > 1.96/√n` (large-sample white-noise band; Bartlett 1946). Passing this
check supports only "no detectable lag-1 dependence", not independence in general.

## 4. Claim types and rules

Conditions: `S_c` claim conditions; `M` = linked metric evidence (`metric_report`
supplied or `outcome_records` computed) with the same metric and population.

| Claim type | Rules (in order of evaluation; all are applied and reported) |
|---|---|
| `metric_value` (descriptive statement about a realised sample; comparator `equals` with tolerance, `at_most`, `at_least`) | M missing → insufficient(missing_evidence). `S_c ⊄ S_M` → insufficient(population_scope). `events > n` or non-finite values → insufficient(integrity). Claimed value differs from recomputed `events/n` (or the supplied value) beyond tolerance, or the realised value violates the comparator → contradict(numerical_validity). Linked limitation with effect `invalidates` affecting this claim type → insufficient(known_limitation). |
| `metric_bound` (inferential bound at confidence `γ`) | As above for missing, scope, integrity, claimed-value mismatch and limitations. Dependence: `temporal`/`clustered` without a dependence-aware interval or design effect → insufficient(temporal_dependence); `unknown` → insufficient(temporal_dependence); declared `independent` but a linked dependence diagnostic is significant → insufficient(temporal_dependence). Bound: `at_most`: `U ≤ b` support, `L > b` contradict, else insufficient(statistical_sample); `at_least` symmetrically. |
| `coverage_guarantee` (prediction-interval coverage ≥ level on a population) | No `calibration` evidence → insufficient(missing_evidence). Claim conditions not all represented in calibration → insufficient(calibration_applicability). Calibration size `n < ⌈1/(1−level)⌉ − 1` → insufficient(statistical_sample). Calibration units dependent or `unknown` → insufficient(temporal_dependence) (exchangeability required; Vovk et al. 2005; Barber et al. 2023). Linked shift diagnostic with `detected = true` → insufficient(distribution_representativeness). Linked realised coverage with `U < level` → contradict. |
| `numerical_adequacy` | System description says no numerical solver → not_applicable. No numerical checks → insufficient(missing_evidence). Any required check `false` → contradict(numerical_adequacy). Any required check absent/null → insufficient(numerical_adequacy). |
| `provenance` | No hash evidence → insufficient(provenance). Referenced file absent → insufficient(provenance). Computed hash ≠ recorded hash → contradict(provenance). Authority weaker than required → insufficient(provenance). |
| `no_leakage` (forecast or i.i.d. task) | Requires feature timing, partition and target-construction evidence; any missing → insufficient(leakage). A predictor available after the prediction origin → contradict(leakage). Overlapping partitions, preprocessing fitted on a non-training split, or (forecast) training not strictly before evaluation → contradict(leakage). A set of predictors that reconstructs the target exactly → contradict(leakage). |
| `independence` (no detectable lag-1 dependence) | No diagnostic → insufficient(temporal_dependence). Significant → contradict(temporal_dependence). |
| `physical_generalization` (from one domain to another) | No passed physical validation in the target domain → insufficient(population_scope); a failed one → contradict. |
| `universal_reliability` (holds under all future shifts) | Always at least insufficient(population_scope): universal quantification over unobserved distributions is beyond any finite evidence. If linked evidence shows `L >` the claimed reliability bound under some evaluated condition → contradict. |
| `operational_risk` (a calibrated risk bound applies to a coming period) | R1: claim condition has fewer than `min_condition_support` calibration units → insufficient(calibration_applicability). R2: recent monitoring statistic exceeds its calibration support maximum → insufficient(distribution_representativeness). R3: recent evidence completeness below requirement → insufficient(missing_evidence). R4: design-effect-adjusted one-sided Clopper–Pearson lower bound of recent risk exceeds the bound → contradict(statistical_sample). Calibration evidence missing → insufficient(missing_evidence). |

Known limitations: a linked `limitation` with effect `invalidates` whose affected
claim types include the claim's type makes it insufficient(known_limitation);
effect `qualifies` is reported without changing the class.

## 5. Baseline C: standard evidence checks

C receives exactly the same JSON and file access. It performs schema validation
(identical parser), missing-evidence warnings, provenance hash checks, and
established metric reporting, but no claim-relative reasoning:

- Missing primary evidence for the claim type → insufficient. Primary evidence:
  metric claims and universal claims — metric evidence; coverage — calibration;
  numerical — numerical checks; provenance — a hash; leakage and independence —
  an attestation; physical generalisation — physical validation; operational
  risk — calibration.
- Any linked hash mismatch → contradicted; referenced file absent → insufficient.
- Metric claims: recompute `events/n`; a claimed-value mismatch or a violated
  descriptive comparator → contradicted. Bounds use a supplied interval if present,
  otherwise an i.i.d. Clopper–Pearson interval (`U ≤ b` supported, `L > b`
  contradicted, else insufficient). Universal claims are treated as `at_most`
  bounds on their reliability bound.
- Coverage: realised coverage below the level → contradicted; otherwise supported.
- Numerical checks: any false → contradicted; any null → insufficient.
- Leakage and independence: attestation true → supported, false → contradicted.
- Physical validation passed → supported, failed → contradicted.
- Operational risk: recent point risk above the bound → contradicted.
- C ignores scope, dependence, shift diagnostics, support and completeness
  checks, authority strength, limitations and not-applicability.

A pre-declared sensitivity variant, **C+scope**, adds rule A4 (scope) to C. It
measures how much of any TRIAD advantage is plain condition matching.

## 6. Examples and counterexamples

- Supported: `metric_value` "accuracy equals 0.6557 ± 0.0001 on the Milestone 6
  logistic test period", evidence 6,106/9,312 correct. The realised value matches;
  dependence does not affect a descriptive statement.
- Insufficient (claim-relative): the same evidence for "accuracy ≥ 0.6 for any
  future electricity period" — conditions not evaluated (A4).
- Contradicted: `metric_bound` "D reduces matched-coverage risk by at least 2
  percentage points", supplied paired whole-sequence bootstrap interval
  [0.035, 0.382] pp — the upper limit excludes the claim (Milestone 3).
- Insufficient although the arithmetic passes: zero errors in 40 i.i.d. audits
  for "error rate ≤ 1%": `U = 0.072`.
- Insufficient although the arithmetic passes: zero errors in 400 consecutive hours
  for the same claim with temporal dependence and no design effect (A5).
- Not applicable: numerical adequacy of a ridge regression without a numerical
  solver.
- Counterexample to the method's power: a feature-timing table that falsely states
  that same-hour temperature is available at the prediction origin. Supplied data
  are not verified against the world; TRIAD will be misled. Such cases are included
  in the validation suite and counted as failures.

## 7. Assumptions

- Claims and evidence are typed truthfully enough for linkage to be meaningful;
  free text is never parsed.
- The Bartlett band and design-effect adjustment are approximations (AR(1)-type
  variance inflation); they can be anti-conservative under long-range dependence.
- Clopper–Pearson bounds condition on the supplied unit definition.
- Rules encode necessary conditions drawn from cited statistical practice; they are
  not sufficient conditions for real-world reliability.

## 8. Falsification conditions

The framework's claimed contributions are falsified (for the tested conditions)
if any of the following is observed:

- F1 (audit utility): on the frozen validation suite, TRIAD's unsupported-claim
  detection rate does not exceed baseline C's by at least 0.20, or TRIAD's false
  rejection rate on valid claims exceeds C's by more than 0.05.
- F2 (predictive utility): in Experiment A, evidence-qualified day decisions do not
  reduce decision cost relative to standard monitoring by the pre-specified
  practical amount with Bonferroni-adjusted confidence for both models.
- F3 (fail-closed property): any input with missing, null or attestation-only
  required evidence is classified as supported (tested adversarially).
- F4 (distinctness): the decisions are reproduced by C plus one simple rule family,
  in which case the contribution is that rule family, not the framework.

## 9. References

- C. J. Clopper and E. S. Pearson. The use of confidence or fiducial limits
  illustrated in the case of the binomial. *Biometrika* 26(4):404–413, 1934.
  https://doi.org/10.1093/biomet/26.4.404
- J. A. Hanley and A. Lippman-Hand. If nothing goes wrong, is everything all right?
  Interpreting zero numerators. *JAMA* 249(13):1743–1745, 1983.
  https://doi.org/10.1001/jama.1983.03330370053031
- M. S. Bartlett. On the theoretical specification and sampling properties of
  autocorrelated time-series. *Supplement to the Journal of the Royal Statistical
  Society* 8(1):27–41, 1946. https://doi.org/10.2307/2983611
- H. R. Künsch. The jackknife and the bootstrap for general stationary
  observations. *Annals of Statistics* 17(3):1217–1241, 1989.
  https://doi.org/10.1214/aos/1176347265
- V. Vovk, A. Gammerman and G. Shafer. *Algorithmic Learning in a Random World*.
  Springer, 2005. https://doi.org/10.1007/b106715
- R. F. Barber, E. J. Candès, A. Ramdas and R. J. Tibshirani. Conformal prediction
  beyond exchangeability. *Annals of Statistics* 51(2):816–845, 2023.
  https://doi.org/10.1214/23-AOS2276
- S. Kapoor and A. Narayanan. Leakage and the reproducibility crisis in
  machine-learning-based science. *Patterns* 4(9):100804, 2023.
  https://doi.org/10.1016/j.patter.2023.100804
- T. Kelly and R. Weaver. The Goal Structuring Notation — a safety argument
  notation. *Proceedings of the DSN 2004 Workshop on Assurance Cases*, 2004.
- National Institute of Standards and Technology. *Secure Hash Standard (SHS)*,
  FIPS PUB 180-4, 2015. https://doi.org/10.6028/NIST.FIPS.180-4
