# Prospective validation and access restrictions

The numerical development configuration is frozen in
`experiments/metrology-numerical-v5.json`. Its four new locations are disjoint
from the ten v4 diagnostic probes. It measures three mesh resolutions across
near-electrode, mid-field and higher-target positions, two layouts, expanded
boundaries and a background-permittivity perturbation. Per-probe results survive
interruption. Agreement is not a bound on continuum error or structural adequacy.

Confirmatory C2/D experiment: **blocked, not frozen or executed**. Freezing sample
sizes or a final practical effect without independent pilot evidence would imply
unsupported readiness. The planning command exposes assumed error rates,
illustrative absolute 0.002 and relative 15% effects, coverage 0.8, sequence size
12 or shared-pool size 48, and correlation 0/0.1/0.5. It uses a normal approximation
and design effect, plus a 24-setting seeded nested-pool simulation of paired error differences.
The simulation varies rates, pool counts, pool sharing and three seeds, while
retaining whole-sequence dependence. It assumes optimistic monotone pairing;
this is not an empirical or crossed-cluster power guarantee. These effects remain
exploratory and have no established scientific utility threshold.

Before accessing a new final test set, a new immutable protocol must specify:

- Primary hypothesis: physics-informed selection improves excessive-error risk
  at 80% retrospective coverage over C2 by a prospectively justified effect.
- Secondary endpoints: calibration-fixed coverage/risk, AURC, ECE/Brier,
  abstention and latency; compare residual-only, constant and random selectors.
- Utility-derived effect and independent pilot estimate; power under paired
  selectors, whole-sequence and shared spatial/material dependence, environmental
  scenarios and multiple simulation seeds. The current approximation is insufficient.
- Numerical mesh, solver, interpolation, boundary, drift and observability evidence,
  with independently evaluated tolerances and unchanged sensor noise.
- Disjoint training/development/calibration/test seeds and spatial/material pools;
  exact sequence counts; calibration algorithm; paired cluster CI method and
  multiplicity policy. Commit the protocol hash before test generation/access.
- Failure: insufficient errors/power, numerical failure, protocol mismatch,
  unexpected missing data or selector loss must block confirmatory support.

The exposed v3 test set is historical evidence, never a new holdout. No final
scientific endpoint is selected from the synthetic software demonstration.
