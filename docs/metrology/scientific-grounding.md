# Scientific grounding and overlap

Established methods, not TRIAD inventions:

1. Yonatan Geifman and Ran El-Yaniv. **Selective Classification for Deep Neural
   Networks.** Advances in Neural Information Processing Systems 30, 2017.
   [Publisher](https://papers.nips.cc/paper_files/paper/2017/hash/4a8423d5e91fda00bb7e46540e2b0cf1-Abstract.html).
   Establishes selective classification and risk/coverage trade-offs. TRIAD does
   not implement or claim that paper's finite-sample risk-control guarantee.
2. Chuan Guo, Geoff Pleiss, Yu Sun and Kilian Q. Weinberger. **On Calibration of
   Modern Neural Networks.** ICML, PMLR 70:1321–1330, 2017.
   [Publisher](https://proceedings.mlr.press/v70/guo17a.html).
   Grounds reliability diagrams and binned calibration diagnostics. Here the
   calibrated event is excessive loss, not necessarily top-class correctness.
3. Yonatan Geifman and Ran El-Yaniv. **SelectiveNet: A Deep Neural Network with
   an Integrated Reject Option.** ICML, PMLR 97:2151–2159, 2019.
   [Publisher](https://proceedings.mlr.press/v97/geifman19a.html).
   Related trained selective prediction; TRIAD evaluates externally supplied
   predictors and does not introduce a new selective learning algorithm.
4. Ran El-Yaniv and Yair Wiener. **On the Foundations of Noise-free Selective
   Classification.** Journal of Machine Learning Research 11:1605–1641, 2010.
   [Publisher PDF](https://www.jmlr.org/papers/volume11/el-yaniv10a/el-yaniv10a.pdf).
   Earlier foundations of the risk/coverage trade-off; its noise-free assumptions
   do not hold automatically for this noisy sensor or logistic demonstration.
5. William L. Oberkampf and Timothy G. Trucano. **Verification and Validation in
   Computational Fluid Dynamics.** Progress in Aerospace Sciences 38(3):209–272,
   2002; Sandia report SAND2002-0529.
   [Author institution](https://www.sandia.gov/research/publications/details/verification-and-validation-in-computational-fluid-dynamics-2002-03-01/).
   Grounds the distinction between code/solution verification and model validation.
   TRIAD uses that distinction: mesh disagreement and solver checks cannot
   substitute for physical validation. No CFD-specific error estimator is claimed.

Sources inspected October 11, 2026. The current
[NIST catalog](https://airc.nist.gov/metrology/) includes Refusal Rate, Test
Fairness (Well-Calibration), and model/task error measures. These overlap in
abstention, calibration and predictive performance. A text search found no entry
named selective risk or Brier, which is not an exhaustive novelty determination.

The proposed contribution is narrower engineering guidance: connect established
selective-risk calculations to explicit numerical/model adequacy and applicability
limitations. Whether this combination adds enough value for catalog inclusion is
unestablished. Prefer a reproducible implementation/reference workflow over a
claim of new mathematics. The physics simulation alone does not demonstrate AI
TEVV applicability; the trained logistic demonstration establishes executable
non-capacitive evaluation only, not transfer of physical assumptions to AI.
