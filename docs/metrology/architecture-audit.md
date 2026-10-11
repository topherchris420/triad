# Milestone 5 architecture and evidence audit

Baseline: `addbf1abbda049f1e1c6e693e38305940d15eac1`, clean clone of master.
`baseline-hashes.json` records SHA-256 of every tracked historical experiment,
result, milestone log and document before implementation. Existing artifacts
are immutable; new studies use new paths and exclusive creation.

Reuse: `forward` assembles conforming P1 electrostatics with Fenris, graded
Spade meshes and sparse Faer LU; `measurements` specifies correlated noise and
whitening; `inverse` supplies a multistart dictionary estimator. `selective`
implements the frozen C2/D study; `observability` diagnoses local information.
`numerical_audit` and `numerical_components` separate mesh and interpolation
disagreement. `research_integrity` enforces the frozen v3 protocol and exclusive
artifact writes. `study_design` currently checks only effect feasibility.

Extend separately: generic labeled prediction evaluation, operational calibration,
multi-cluster descriptive uncertainty, bounded measurement-driven refinement,
three-resolution numerical diagnostics and prospective planning. Do not change
the historical solver or frozen v3 selector to make old results look better.

Existing tests cover affine/layered analytic solutions, conservation, GLS
whitening, inverse recovery on synthetic signals, interpolation, protocol
mutation rejection and artifact hashes. Those are software/numerical checks,
not validation of the electrostatic model against physical reality.

Preserved findings: baseline 20/1,080 errors above 5 mm, ten despite convergence;
v3 primary hypothesis unsupported and numerical readiness failed; v4 maximum
interpolation discrepancy reduced 92.4% on ten development probes, while mesh
disagreement still reached 9.821 whitened units. No 3D or hardware validation.
The v4 stencil used diagnostic truth and was not operational inference.

Not established: numerical error bounds, calibrated physical uncertainty,
general superiority of physics-informed selection, adequate prospective power,
or a novel mathematical metric. The historical tests and experimental records
were inspected before new code was added.

Baseline CI verified successful: [Rust verification run 38163881809](https://github.com/topherchris420/triad/actions/runs/38163881809), matching the baseline revision.
