# Known failure modes

- An optimizer may converge to an incorrect estimate. Convergence never replaces
  outcome evaluation or physical adequacy evidence.
- A small interpolation discrepancy may coexist with excessive FEM discrepancy.
  A 0.25 mm solve is a finer reference, not ground truth.
- Electrode edges mix boundary conditions and generate singular fields. P1
  convergence need not follow a simple global power law. Target polygon resolution
  also changes with mesh spacing. The historical default graded mesh remains unchanged. An optional electrode-edge
  and target-interface candidate is evaluated separately; no globally optimized
  mesh is claimed.
- Bounded measurement-driven cell ranking can miss modes; center ranking is
  particularly vulnerable to narrow likelihood basins. The adaptive API exposes
  all sampled candidates and cost, but always abstains until adequacy is established.
  Its triangle envelope measures dictionary-versus-callback discrepancy only,
  not an error probability or physical bound. Cache is local to one call.
- An apparently calibrated score can fail under concept shift. The trained demo
  intentionally reverses feature effects without refitting.
- Whole-sequence bootstrap understates uncertainty if sequences share material
  or spatial pools. Separate spatial bootstrap is sensitivity analysis, not a
  multiway solution. Zero-event bootstrap cannot establish a rare-event upper bound.
- IDs cannot prove dataset independence. External adapters must document actual
  training provenance and shared sampling pools. Missing labels prevent full risk
  estimation; no missing-at-random assumption is silently imposed.
- Deterministic retrospective tie-breaking can affect matched-coverage risk;
  the curve instead uses whole tie groups. Neither yields deployable test thresholds.
- An evidence link and a passing attestation can be wrong. The evaluator does not
  audit linked evidence. Physical-confidence validation is never automatically set.
- No 3D, hardware, clinical or other domain validation follows from these examples.
