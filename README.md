# TRIAD

**Vers3Dynamics** — principal investigator **Christopher Woodyard**.
*Resonance as Substrate. Intelligence as a Layer.*

Independent Rust research prototype investigating when sparse capacitive
localization becomes unreliable under environmental and material uncertainty.
This repository implements **Milestones 0–2**: commercial-reference audit,
validated 2D electrostatics, reproducible simulated observations, conventional
joint position/material estimation, and localization/ambiguity diagnostics.
No uncertainty-aware selective method or commercial performance comparison is
implemented. The hypothesis remains untested.

The user requested Rust after the initial Python scaffold. All maintained
computation, data generation, inversion, validation and plotting are Rust;
Python is not required. The discarded scaffold's failed numerical report is
retained transparently. No Infineon source or archive is redistributed.

## Reproduce

With Rust and a compatible native linker installed:

```sh
cargo build --release --locked
cargo test --release --locked
cargo run --release --locked -- validate
cargo run --release --locked -- baseline
```

`baseline` requires successful validation with the exact configuration hash.
`all` runs validation then baseline. Locked dependencies include Fenris 0.0.33
for finite element assembly, Spade 2.15.1 for constrained mesh generation, and
faer 0.24.4 for sparse linear algebra. No custom FEM engine is included.

On this Windows ARM64 host the tested runtime is x86_64 GNU under emulation,
Rust 1.99.0, with portable GCC w64devkit 2.10.0. The supplied user-local helper
configures the installed linker and Rust's bundled GNU runtime:

```powershell
.\scripts\build.ps1 build
.\scripts\build.ps1 test
.\target\release\triad.exe validate
.\target\release\triad.exe baseline
```

The helper expects toolchain locations beneath USERPROFILE; conventional MSVC,
Linux and macOS Rust setups can use cargo directly. Cargo.lock freezes library
versions. Numerical values should match within roundoff across platforms;
runtime/memory and optimizer tie choices may vary. No bitwise cross-platform
guarantee is made.

## Evidence and outputs

- [Research status and actual results](docs/research-status.md)
- [Physical model, units, excitation and noise equation](docs/physical-model.md)
- [Frozen experiment protocol](docs/experiment-protocol.md) and [amendments](docs/protocol-amendments.md)
- [CAPSENSE architecture audit](docs/commercial-reference-analysis.md) and [algorithm evidence](docs/algorithm-evidence.md)
- [REA capabilities and actual MCP evidence](docs/rea-capability-audit.md)
- [Targeted literature review](docs/literature-review.md) and [limitations](docs/limitations.md)
- `results/validation.json`: actual physical validation gates.
- `results/reproducibility-check.json`: verified byte-identical dataset and CSV replay after fixing numeric cache round trips.
- `results/dataset.json`: observations, empty histories, truth in clearly named fields, split and sequence IDs.
- `results/predictions.json`: saved before evaluating truth.
- `results/localization.csv`, `evaluation.json`, `summary.json`: measured baseline errors and whole-sequence intervals.
- `results/ambiguity.json`, `sensitivity.json`: finite-grid physical diagnostics.
- `results/plots/*.svg`: standalone scientific plots.
- `results/cache/`: metadata-keyed FEM dictionaries and refined physical pools.

## Source layout

`src/forward.rs`, `measurements.rs`, `inverse.rs`, `observability.rs`,
`evaluation.rs`, `validation.rs`, `experiment.rs`, and `plots.rs` comprise the
library; `main.rs` provides the command line. `tests/numerics.rs` exercises
analytic physics, capacitance structure, noise, GLS, data isolation and inverse
recovery, plus exact float cache round trips (nine tests). `experiments/config.json` fixes seeds, geometry, meshes, scenarios,
the 5 mm tolerance and future selective endpoints before results.

The model represents an infinitely extended cylindrical 2D cross-section, not
a finite 3D coplanar commercial sensor. Measurements are **pF/m**, not pF.
Local sensitivity and finite-grid ambiguity are diagnostics, not proof of
global identifiability. Later milestones need new untouched test sequences.
