# Targeted literature review and novelty boundary

This is a targeted conceptual review for Milestones 0–2, not an exhaustive
priority search or a claim of original methodology. Sources were inspected
before final localization predictions. The question is whether observability
adds practical predictive ranking value beyond the posterior's estimated
error probability when the assumed likelihood is misspecified by drift.

| Topic | Primary reference | Implication for TRIAD |
|---|---|---|
| Capacitive position FEM | [COMSOL capacitive position sensor model](https://doc.comsol.com/6.3/doc/com.comsol.help.models.acdc.capacitive_position_sensor/capacitive_position_sensor.html) | Dielectric contrast alters electrode coupling; FEM position sensing is established. TRIAD's finite 2D domain is a computational study, not a reproduction of that commercial example. |
| Sparse capacitance inverse problems | [Yang and Peng, 2003](https://doi.org/10.1088/0957-0233/14/1/201) | ECT has nonlinear soft-field coupling, limited measurements and ill-conditioning. Their distributed imaging problem differs from TRIAD's fixed-radius single-target parameterization; ill-posedness cannot simply be transferred as a proof. |
| Bayesian inverse problems | [Stuart, 2010](https://doi.org/10.1017/S0962492910000061) | Priors and likelihood define posterior uncertainty. A wrong material prior or drift model can produce misleading confidence even when numerical inversion succeeds. Multimodality must be retained in later milestones. |
| Selective risk and coverage | [Geifman and El-Yaniv, 2017](https://arxiv.org/abs/1705.08500) | Abstention trades coverage for risk. Their classification setting motivates matched-coverage comparisons; reducing acceptance alone is not evidence of better regression reliability. |
| Nuisance-adjusted Fisher information | [Shen and Win, 2010](https://winslab.lids.mit.edu/wp-content/papercite-data/pdf/shewin-j10a.pdf) | Equivalent information eliminates nuisance blocks via a Schur complement. This mathematical principle is established; its use for dielectric nuisance in this geometry would not alone demonstrate novelty. Local information does not establish global uniqueness. |
| Shifted uncertainty calibration | [Tibshirani et al., 2019](https://arxiv.org/abs/1904.06019), [Barber et al., 2023](https://arxiv.org/abs/2202.13415) | Calibration validity depends on distribution assumptions. Covariate-shift weighting requires density-ratio information; arbitrary unobserved gain/baseline drift is not automatically covered. |
| Capacitive drift compensation | [Lightweight thermal compensation of capacitive MEMS accelerometers, 2021](https://pmc.ncbi.nlm.nih.gov/articles/PMC8124870/) | Sensor drift compensation has established physical and empirical precedents. This sensor class differs from coplanar target localization, so its measured compensation performance is not a TRIAD baseline. |
| Physics-informed inverse uncertainty | [Efficient Bayesian PINNs via ensemble Kalman inversion, 2023](https://arxiv.org/abs/2303.07392) | Combining physics and probabilistic inversion already exists. TRIAD does not need a neural PDE surrogate for its small inverse state. |

Decision-theoretic interpretation: if C2's posterior is correctly specified,
ranking by its expected tolerance-exceedance loss is the Bayes-optimal selection
for that loss at fixed acceptance. Adding a deterministic physics diagnostic
computed from identical information cannot automatically improve this optimum.
An improvement could arise under misspecification or numerical approximation;
then it must be attributed to the specific difference, not to a vague benefit of
physics. This is reasoning about the experimental hypothesis, not a measured
result. The proposed method may perform worse.

Remaining review needs before a manuscript: a broader sparse planar capacitive
localization search, regression selective prediction, posterior expected-loss
selection, identifiability with dielectric/material uncertainty, calibration
under time-dependent latent drift, and hardware measurement practice. No claim
of novelty is supported by the present audit or baseline study.
