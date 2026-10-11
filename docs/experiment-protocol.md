# Protocol registered before localization results

Vers3Dynamics; principal investigator Christopher Woodyard. Scope of this run:
Milestones 0–2, fixed geometry, conventional always-return estimator only.
The machine-readable configuration is `experiments/config.json` (version 2,
Rust target-conforming FEM). Amendments are in `protocol-amendments.md`. It is written
before any final observations or localization metrics are generated. This is a
local preregistration, not an externally timestamped public registration.

Primary question for later milestones: does nuisance-aware physical observability
improve rejection of unreliable position predictions under unseen drift? Null:
no meaningful improvement over calibrated conventional uncertainty methods.
No selective method or novelty claim is tested in this run.

Future primary endpoint: probability of position error exceeding **5 mm among
accepted predictions**, comparing D against C2 under strong unseen drift at
matched 80% test coverage. A 2 percentage point absolute decrease is the declared
practical improvement threshold, with a paired whole-sequence bootstrap interval
excluding zero. This ranking evaluation uses test labels retrospectively and is
not threshold calibration. Separately, freeze calibration-only thresholds at
60%, 80%, and 90% nominal coverage and report actual shifted test coverage/risk.
Incorrect-acceptance frequency among all samples equals coverage times risk;
report both. Future requirements also include accepted p90 error, AURC, region
coverage, uncertainty calibration, runtime, memory, and unfavorable findings.
Later selective implementation requires a new frozen version and untouched
new test sequences: the baseline results here are already inspectable.

Milestone 2 baseline endpoint: mean, median, p90 Euclidean localization error,
fraction over 5 mm, and position/material/drift-dependent error. Test coverage is
100%; no abstention is implemented. GLS jointly estimates x, y, and relative
target permittivity in [2,8], from ten channels, using five separated multistarts
on a piecewise multilinear dictionary. Empty-target histories are the only
per-sequence calibration information. Training and calibration split sequences
exist for later work; no fitted model hyperparameters use their labels here.

Three fixed seeds, each scenario has 2 training, 2 calibration, 3 test sequences,
12 frames per sequence and 24 empty-target calibration frames. Independent
Uniquely derived u64 ChaCha8 seeds identify every complete sequence. All final test
observations use a finer mesh than the dictionary and continuous off-grid target
design pools (the dictionary-selected ambiguity pool is the exception). Fixed
physical pools are shared across split sequences as a conditional experimental
design; noise, histories, and drift are independent. Predefined scenarios: clean, mild drift, strong drift, material outside
[2,8], combined material/drift, high target region (poor sensitivity), finite-grid
near-ambiguous states, correlated/channel-specific disturbance, and +10% target
radius misspecification. Ambiguity selection uses only forward dictionary
information, never noisy test labels.

Numerical gates, before scientific performance: relative symmetry <=1e-10;
charge conservation and reaction/energy consistency <=1e-9; analytic affine and
layered dielectric benchmarks <=1e-9; exact target/background equality; total C
mesh change <=4%, target-induced channel-vector mesh change <=8% from 1 to 0.5 mm,
and <=4% from 0.5 to 0.25 mm at four fixed probe states; signal domain-size
sensitivity <=20% when the grounded enclosure grows from 90 to 120 mm. That last
bound defines a finite-enclosure study, not an open-domain convergence claim.
Mesh errors must also be reported relative to measurement noise; passing a
relative gate does not imply that numerical error is negligible at every signal
level. No post-result relaxation of gates.

Stopping: stop performance generation on a failed forward gate. Repair numerical
implementation or refine the mesh with a documented protocol amendment rather
than hide failure. After one complete prescribed baseline run and tests, report
limitations and stop at Milestone 2. Do not extend to electrode optimization,
selective algorithms, 3D hardware claims, or a manuscript without new evidence.
