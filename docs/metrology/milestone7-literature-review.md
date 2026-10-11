# Milestone 7 literature and implementation review

The comparator choices recorded here were fixed in the frozen protocol (commit
`bb1bd0f`) before implementation; this write-up was completed after the
experiments and does not change them. Every reference below
is in [`milestone7-references.json`](milestone7-references.json) and was
machine-checked by `scripts/check-references.py` (DOI resolution and registered
title, or page title for URLs). That check shows that each source exists and its
title matches. It does not show that each source supports the statement made here;
those statements were written from the cited works.

## 1. What already exists

| Area | Established methods | Status in TRIAD before M7 | Used in M7 as |
|---|---|---|---|
| Selective prediction | Reject option and error–reject trade-off (Chow 1970); risk–coverage and selective classification (El-Yaniv & Wiener 2010; Geifman & El-Yaniv 2017) | Implemented (`metrology.rs`): calibration-quantile threshold, risk, coverage, AURC | Conventional evaluation (A) |
| Probability calibration | Brier score (Brier 1950); reliability diagrams and ECE (Guo et al. 2017) | Implemented for the error event | Conventional evaluation (A) |
| Conformal prediction | Split/inductive conformal regression (Papadopoulos et al. 2002; Vovk et al. 2005); locally weighted (normalised) intervals (Lei et al. 2018); tutorial (Angelopoulos & Bates 2023) | Not implemented | Normalised split conformal, measured on test (A) |
| Conformal under shift and risk control | Weighted/beyond-exchangeability bounds (Barber et al. 2023); adaptive conformal inference with online updating (Gibbs & Candès 2021); conformal risk control (Angelopoulos et al. 2024) | Not implemented | Reviewed; not implemented (see §3) |
| Shift detection | Dataset-shift taxonomy (Quiñonero-Candela et al. 2009); black-box shift detection (Lipton et al. 2018); empirical comparison favouring univariate two-sample tests (Rabanser et al. 2019); Kolmogorov–Smirnov (Massey 1951) | Mean-shift window scores (M6) | Max univariate KS of history features, previous day vs development (B, `cov`) |
| Sequential and performance monitoring | CUSUM (Page 1954); drift detection from error rates (Gama et al. 2004); adaptive windowing (Bifet & Gavaldà 2007); survey (Gama et al. 2014) | Not implemented | Previous-day realised accepted error rate (B, `perf`) |
| Dependent-data uncertainty | Moving-block bootstrap (Künsch 1989); stationary bootstrap (Politis & Romano 1994); white-noise band (Bartlett 1946) | Circular moving blocks (M6) | Paired circular moving-block bootstrap of days; lag-1 diagnostics |
| Rare events and exact intervals | Clopper & Pearson (1934); zero-numerator "rule of three" (Hanley & Lippman-Hand 1983) | Not used | Evidence rules |
| Forecast evaluation | Scale-free accuracy, naive benchmarks (Hyndman & Koehler 2006) | — | Persistence and seasonal-naive references (A) |
| Leakage | Formulation and detection (Kaufman et al. 2012); taxonomy of leakage in ML-based science (Kapoor & Narayanan 2023) | Feature exclusions documented by hand | Mechanical feature-timing, partition and target-construction rules (D) |
| Reporting and validation checklists | Model cards (Mitchell et al. 2019); datasheets (Gebru et al. 2021); REFORMS (Kapoor et al. 2024); TRIPOD+AI (Collins et al. 2024) | Free-text limitations | Basis of baseline C (missing-evidence and attestation items) |
| Assurance and risk frameworks | Claims–arguments–evidence structure in assurance cases (Goal Structuring Notation; Kelly & Weaver 2004); NIST AI RMF (NIST AI 100-1, 2023); measurement uncertainty (JCGM 100:2008) | Not used | Conceptual framing only; the AI RMF is a mapping framework, not grounding |
| Integrity | Secure Hash Standard (FIPS 180-4) | SHA-256 manifests | File and embedded-artifact hash rules |

## 2. Comparator mapping for the Milestone 7 experiments

Every method receives the same frozen predictions, timestamps, calendar, labels as
they arrive, and computing resources.

- **A. Conventional evaluation.** MAE, RMSE, bias, material-error rate, persistence
  and seasonal-naive references, calibration-fixed selective risk and coverage,
  AURC, Brier, ECE, and normalised split-conformal coverage.
- **B. Strong established monitoring.** Label-free covariate monitoring (`cov`,
  univariate KS following Rabanser et al. 2019), label-free confidence monitoring
  (`conf`), and delayed-label performance monitoring (`perf`, in the spirit of
  Gama et al. 2004). Labels arrive one hour after each target, so `perf` is
  operationally available at the start of the next day.
- **C. Standard evidence checks.** In Experiment A: the maximum of normalised
  `cov` and `perf` plus a completeness warning (`std`). In Experiment B: schema
  validation, missing-evidence warnings, hash verification, recomputation of the
  claimed metric with supplied or i.i.d. Clopper–Pearson intervals, and acceptance
  of checklist attestations, as in common reporting checklists.
- **D. TRIAD.** Claim-relative evidence rules
  ([method](evidence-sufficiency-method.md)) applied to the same information.

## 3. Reviewed but not implemented, and why

- **Adaptive conformal inference** (Gibbs & Candès 2021) updates the miscoverage
  level online from realised coverage. It is a strong, established answer to
  temporal shift in interval coverage. It was not implemented because the
  Milestone 7 question is about decisions to trust a model for a period, not about
  maintaining interval coverage. It remains a natural comparator for future work.
- **Weighted conformal and conformal risk control** (Barber et al. 2023;
  Angelopoulos et al. 2024) give guarantees under stated conditions; the hourly
  bike stream is dependent and seasonal, so their exchangeability-type assumptions
  would have to be argued separately.
- **CUSUM / ADWIN** detectors (Page 1954; Bifet & Gavaldà 2007) were represented
  by the simpler previous-day rate (`perf`). A tuned sequential detector might
  outperform it; that strengthens, not weakens, the conclusion that TRIAD showed
  no predictive advantage.

## 4. Novelty assessment

- Combining selective risk, calibration, shift diagnostics and validity notes is
  **not novel**; each component is standard and the combination resembles ordinary
  model-evaluation practice.
- Classifying a claim against evidence is the core idea of **assurance cases**.
  TRIAD's framework is a small, machine-executable instance specialised to
  predictive-reliability claims (typed claims, typed evidence, fail-closed axioms,
  explicit statistical rules). Whether that executable specialisation is a
  distinct methodological contribution beyond assurance-case practice and
  checklists such as REFORMS was **not established** by Milestone 7. Experiment B
  measured detection on an author-constructed suite only.
- The TRIAD day policy in Experiment A uses a design-effect-adjusted
  Clopper–Pearson contradiction test and calendar-support rules. Both are
  textbook constructions, not new statistics.

## References

Full citations with DOIs/URLs are in [`milestone7-references.json`](milestone7-references.json).
