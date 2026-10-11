# TRIAD

**When should a sensor refuse to answer?**

[![Rust verification](https://github.com/topherchris420/triad/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/topherchris420/triad/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-0B5D63.svg)](LICENSE)

[Start here](#start-with-the-evidence) · [Recorded results](#baseline-results-the-system-is-accurate-in-some-places-and-fragile-in-others) · [New study](#new-research-code-test-before-claiming) · [Evidence map](#evidence-map)

*An independent Vers3Dynamics research instrument for sparse capacitive localization, physical observability, and the limits of confident inference.*


> A precise measurement is not necessarily a trustworthy conclusion.

Capacitive sensors register the world through changes in an electric field. Four electrodes might yield ten distinct self/mutual capacitance measurements, yet multiple positions, dielectric properties, and environmental conditions can still explain what was observed. TRIAD asks what happens when a localization algorithm still reports one answer.

The research is deliberately staged. First model the field. Then invert the measurements. Then, only when those foundations withstand numerical and statistical checks, investigate whether physics-informed uncertainty can identify the moments when the evidence is insufficient.

**No hardware performance, commercial superiority, or original selective-prediction method is claimed.**

![Recorded TRIAD baseline scenario comparison](results/plots/scenario-comparison.png)

*Plot generated from the frozen conventional-estimator simulation. These are localization errors, not improvements from selective prediction.*

## Start with the evidence

TRIAD is a **Rust research instrument**, with a frozen simulation baseline and a separate, unexecuted selective-prediction study. The useful result so far is a distinction: **an optimizer's stopping flag is not a measure of whether its answer is trustworthy**.

```sh
git clone https://github.com/topherchris420/triad.git
cd triad
git switch main
cargo test --locked
cargo run --locked -- --help
cargo run --locked -- audit
```

The audit reads committed predictions and writes a derived report; no expensive FEM experiment is needed to inspect the baseline. Start with [research status](docs/research-status.md) for findings, [the physical model](docs/physical-model.md) for assumptions, or [the selective protocol](docs/selective-v3.md) to work on the next experiment.

A built binary also accepts `--root /path/to/triad` so you can explicitly select a separate repository/data copy. Full baseline commands replace baseline outputs; preserve the committed evidence before running them.

## The question, in equations

The current experiment solves an idealized **2D electrostatic cross-section**:

$$
\nabla\cdot\left(\varepsilon(\mathbf{r})\,\nabla\phi(\mathbf{r})\right)=0.
$$

Four boundary electrodes are excited in turn; reaction charges and energy integrals construct a Maxwell capacitance matrix. Ten unique entries form the measured vector, expressed in **pF/m** (capacitance per unit length), with a target-induced signal formed by subtracting a same-mesh empty reference.

The simulated observation process introduces unknown dielectric properties, correlated noise, and time-varying drift:

$$
\mathbf{z}_t =
\mathrm{diag}\!\left(\mathbf{1}+\mathbf{g}_t\right)
\,\mathbf{f}(x_t,y_t,\varepsilon_{r,t})
+\mathbf{b}+\mathbf{d}_t+a_t\mathbf{1}+\boldsymbol{\eta}_t.
$$

The implemented estimator minimizes a noise-normalized residual over **position and unknown relative permittivity**. It always returns a point, even when an optimizer stopping condition is not met.

The newly implemented, **not yet scientifically evaluated**, selective experiment asks whether a position should be accepted when uncertainty and physics-based observability indicate that it may be wrong. Its principal future comparison is against *posterior expected-error rejection*, not merely an always-answer baseline:

$$
R(\tau)=
\mathbb{P}\!\left(
\left\lVert\hat{\mathbf{x}}-\mathbf{x}\right\rVert>5\,\mathrm{mm}
\,\middle|\,s(\mathbf{z})\geq\tau
\right).
$$

$$
C(\tau)=\mathbb{P}\!\left(s(\mathbf{z})\geq\tau\right).
$$

A reduction in risk caused merely by refusing more predictions is not a discovery. The proposed primary comparison holds **acceptance coverage at 80%** and separates retrospective ranking from a deployable threshold calibrated without test labels.

## What exists today

| Research layer | Implementation | Evidential status |
| :--- | :--- | :--- |
| Industrial reference review | Infineon CAPSENSE source/contract audit | **Documented**, not copied or benchmarked |
| Reverse-engineering instrumentation | REA MCP capability audit | **Connected**, no commercial binary analyzed |
| Forward physics | Four electrodes, dielectric target, P1 FEM, capacitance matrix | **Implemented**, passes frozen numerical gates |
| Simulated observations | Unknown material, baseline calibration, correlated noise and drift | **Implemented**, synthetic only |
| Inverse estimation | Bounded multistart generalized least squares over x, y, and permittivity | **Implemented**, convergence limitations recorded |
| Ambiguity diagnostics | Finite dictionary alternative search and position sensitivity | **Implemented**, not global identifiability proof |
| Nuisance-adjusted local information | Noise-whitened position Jacobian, dielectric Schur complement | **Implemented as a diagnostic**, not an abstention rule |
| Frozen evidence | Saved datasets, predictions, metrics, plots, protocol amendments | **Recorded** and reproducible on the tested host |
| Retrospective optimizer audit | Dedicated read-only analytical command | **Implemented**, generated report is exploratory |
| Posterior C2 / physics-informed D / probability calibration | Versioned new Rust study | **Implemented, not yet executed or validated** |
| Electrode geometry screening | Same four-channel-array width and 10 observable entries; new independent holdout | **Implemented as exploratory code; no result claimed** |
| 3D fields, fabricated sensors, hardware trials | Outside the current study | **Not performed** |

### Baseline results: the system is accurate in some places and fragile in others

The locked pilot contains **2,520 synthetic observations** across **210 independent sequences**; **1,080** observations belong to the test split. There are ten scenario families, nine test sequences per family, and 12 frames per sequence. These figures come from the committed baseline artifacts, not from a commercial chip.

| Test scenario | Mean localization error | 90th percentile | Error above 5 mm |
| :--- | ---: | ---: | ---: |
| Clean | 0.817 mm | 1.525 mm | 0/108 |
| Strong unseen drift | 1.360 mm | 3.010 mm | 2/108 |
| Drift + material mismatch | 1.407 mm | 3.254 mm | 6/108 |
| Low sensitivity | 2.192 mm | 3.659 mm | 1/108 |
| Near-ambiguous states | 1.813 mm | 3.370 mm | 2/108 |
| Disturbed individual channel | 1.335 mm | 3.916 mm | 6/108 |

Other test scenarios and sequence-level intervals appear in [the recorded research report](docs/research-status.md). The localization problem becomes harder under some shifts, but the current 5 mm error event is **rare**, especially in the designated strong-drift scenario. A later experiment must account for that low event rate before making selective-risk claims.

![Recorded ambiguity diagnostics](results/plots/ambiguity.svg)

*Finite-dictionary near-collisions, not a proof of globally indistinguishable physical targets.*

### A finding worth taking seriously

The frozen pilot contains **451 nonconvergence flags among 1,080 returned positions**. Only **10 of the 20 errors above 5 mm** are flagged; the other ten have a successful convergence flag.

That distinction is crucial: **optimizer convergence is not measurement confidence**. An excellent optimization status can accompany an incorrect physical interpretation. Conversely, a flagged optimizer can return an accurate position.

Run the new retrospective audit against the *unchanged* saved evaluation file:

```sh
cargo run --locked -- audit
```

It writes `results/optimizer-audit.json` and records the evaluated source-file hash. It does **not** modify the frozen baseline, calibrate an abstention score, or substantiate the primary hypothesis. See [the optimizer-audit interpretation](docs/optimizer-audit.md).

## How the instrument works

```mermaid
flowchart TD
    A["Four electrodes · 2D electrostatics"] --> B["Ten target-induced capacitance channels"]
    B --> C["Drift, gain, correlated noise and calibration"]
    C --> D["Shared bounded inverse estimator"]
    D --> E["Frozen baseline: point errors and optimizer audit"]
    D --> F["New study: posterior C2 and physics-informed D"]
    F --> G["Development: probability calibration"]
    F --> H["Calibration: acceptance thresholds"]
    G --> I["Independent test: risk, coverage and reliability"]
    H --> I
```

The 2D world consists of four zero-thickness 8 mm electrode strips on the lower boundary of a grounded finite enclosure. A 4 mm radius dielectric disk represents a cylinder extending without end in the omitted direction. The model is *not* a physically complete finite 3D coplanar sensor and does not include substrates, real readout electronics, dispersive losses, or parasitic capacitances.

Implementation: [Fenris](https://crates.io/crates/fenris) for P1 finite-element assembly, [Spade](https://crates.io/crates/spade) for conforming meshes, and [faer](https://crates.io/crates/faer) for sparse linear algebra. Numerical data are produced without Python or a bespoke FEM engine.

### Numerical validation and its limits

Recorded numerical tests include analytical affine/layered-dielectric benchmarks, Maxwell-matrix reciprocity, charge conservation including the enclosing ground, reaction/energy agreement, mesh refinement, target/background equality, and finite-enclosure sensitivity.

**All prescribed gates passed** for the tested probes, but passing relative mesh tolerances does not mean approximation errors are smaller than sensor noise. At one near-electrode probe, the 1 mm-to-0.5 mm mesh signal change was **14.47 times the nominal single-channel noise sigma in Euclidean norm**. The grounded-domain target-induced signal changed by up to **13.23%** at the tested probes when the enclosure was enlarged. Both matter when interpreting likelihoods and confidence.

The [physical-model definition](docs/physical-model.md), [raw validation values](results/validation.json), and [limitations](docs/limitations.md) state exactly what is and is not established.

## Reproduce the locked baseline

Rust and a compatible native linker are required. With a conventional Linux/macOS/Windows Rust environment:

```sh
cargo build --release --locked
cargo test --release --locked
cargo run --release --locked -- validate
cargo run --release --locked -- baseline
cargo run --release --locked -- audit
cargo run --release --locked -- information
```

- `validate`: performs the **numerical physics gates** and records `results/validation.json`.
- `baseline`: generates the locked dataset, fits the conventional estimator, and writes results. Refuses to run without matching validation.
- `audit`: performs a **retrospective descriptive analysis** of the already recorded evaluation, without rerunning estimation.
- `information`: computes a **local Fisher-information diagnostic** with unknown dielectric treated as a nuisance parameter, using saved dictionary-cell midpoints. It does not test a selector or read true test locations.
- `all`: validation, baseline, then optimizer audit. This may be compute-intensive without cached FEM dictionaries.

**Preserve the committed outputs before rerunning**. The pilot already has observed test labels, so any optimizer changes or new hypothesis tests require a **new protocol and fresh untouched test sequences**, not a quiet replacement of the saved evidence.

For the recorded Windows ARM64 environment, an x86-64 GNU Rust toolchain runs under emulation. The project includes a user-local `scripts/build.ps1` helper for that toolchain, if installed:

```powershell
.\scripts\build.ps1 build
.\scripts\build.ps1 test
.\target\release\triad.exe audit
```

The historical Windows log records nine numerical tests. The current suite additionally checks inference, statistical diagnostics, protocol drift, artifact preservation and CLI behavior. GitHub Actions runs the locked suite, checks formatting and recomputes derived audits while checking that frozen baseline files remain unchanged. It does not rerun the full expensive FEM experiment on each commit.

## Independent references, not dependencies

**[Infineon CAPSENSE](https://github.com/Infineon/capsense)** informs the architectural literature review of baseline tracking, thresholds, filtering, and position processing. Its vendor license is restrictive. No CAPSENSE source, firmware, or compiled library is redistributed in TRIAD. The reference work is not a head-to-head performance benchmark.

**[REA: Reverse Engineer Anything](https://github.com/morluto/rea)** is available through MCP for lawful, authorized investigation. TRIAD's audit confirmed an active REA service, **not** a working native decompiler on the recorded Windows ARM64 configuration. No commercial binary was decompiled. The scientific model does not require REA.

References and limits: [commercial source audit](docs/commercial-reference-analysis.md) · [algorithm evidence](docs/algorithm-evidence.md) · [REA capability audit](docs/rea-capability-audit.md) · [prior-art review](docs/literature-review.md).

## New research code: test before claiming

The experimental Milestone 3 code now provides numerical auditing, posterior C2, a preregistered physics-informed D score, probability calibration, drift-only selective risk evaluation, independent sequence-level bootstrap intervals, and exploratory electrode-layout screening. **These are implementations, not findings.** No new outcome is claimed until commands are executed, numerical discrepancies are addressed, and a fresh test has enough failure events.

```sh
cargo test --locked
cargo run --release --locked -- numerical
cargo run --release --locked -- selective
cargo run --release --locked -- geometry-screen
```

**Run in this order.** `numerical` quantifies noise-normalized mesh, interpolation, and optimizer error on six development probes. `selective` creates independently seeded development, calibration, and test drift sequences, writes scores before test-truth evaluation, and compares C2 and D at exactly 80% retrospective test acceptance as well as independently calibrated deployment thresholds. It reports Brier/reliability/ECE and paired whole-sequence bootstrap intervals, including null or adverse findings. `geometry-screen` screens five equal-width, four-electrode layouts on an independent development and holdout split; **this is not proof of improved selective risk**.

The old baseline is intentionally never rerun by these commands. Study artifacts use distinct, versioned paths. Every executable protocol setting is checked before running. An existing selective run directory—including an interrupted run—is preserved; numerical and geometry reports also refuse replacement. The selective summary is written last and records hashes of its score, evaluation and run files. Geometry screening checks those hashes before proceeding. See [research integrity and interrupted runs](docs/research-integrity.md). Results will not be embedded into the README until verified.

See [the complete Milestone 3 research protocol, limitations and reproducible commands](docs/selective-v3.md), the [versioned machine-readable protocol](experiments/selective-v3.json), and [the original next-experiment gate](docs/next-experiment.md).

## The next experiment: earn the right to abstain

The next stage is **not** to bolt on a confidence percentage. It is to establish a stronger optimization reference, quantify mesh and surrogate-model mismatch in the same units as noise, then pre-register a new powered C2-versus-D comparison. All methods must have the same information budget and independent test sequences.

1. **Audit numerical inference.** Separate projected/boundary stopping conditions from genuinely suboptimal fits; quantify surrogate-interpolation and mesh error.
2. **Test meaningful ambiguity.** Account for unknown dielectric and restricted drift nuisance parameters, as well as near-identical measurements from distant states.
3. **Evaluate strong selectors.** Posterior expected-error rejection (**C2**) and a fixed physics-informed rejection score (**D**) now have experimental code. Residual-only and posterior-variance comparators remain future extensions.
4. **Freeze new tests.** Calibrate thresholds on calibration sequences and evaluate deployment coverage on unseen sequences. Also report matched-coverage risk without pretending it is a deployable threshold.
5. **Accept the null.** If C2 matches or outperforms D, say so. A sophisticated observability score cannot manufacture information absent from the electrodes.

Read [the next-experiment plan](docs/next-experiment.md). The governing [original protocol](docs/experiment-protocol.md) and [documented amendments](docs/protocol-amendments.md) remain intact.

## Evidence map

| For this question | Read |
| :--- | :--- |
| What has actually been measured? | [Research status](docs/research-status.md), [summary JSON](results/summary.json), [scenario comparison](results/plots/scenario-comparison.png) |
| How is electrostatics implemented? | [Physical model](docs/physical-model.md), [forward source](src/forward.rs), [validation JSON](results/validation.json) |
| How are positions estimated? | [Inverse source](src/inverse.rs), [original predictions](results/predictions.json) |
| How are ambiguity and sensitivity assessed? | [Observability source](src/observability.rs), [finite-grid ambiguity](results/ambiguity.json) |
| Can I reproduce it? | [Protocol](docs/experiment-protocol.md), [amendments](docs/protocol-amendments.md), [replay audit](results/reproducibility-check.json) |
| Where could this fail? | [Limitations](docs/limitations.md), [optimizer audit](docs/optimizer-audit.md), [next experiment](docs/next-experiment.md) |

The first cached rerun exposed a floating-point JSON round-trip issue; it was corrected without changing scientific parameters. The corrected run reproduced the dataset and localization CSV byte-for-byte on the recorded host. Failure evidence is retained rather than erased.

### Scope, authorship, and reuse

TRIAD is independent research by **Christopher Woodyard, Vers3Dynamics**.

TRIAD is released under the [MIT License](LICENSE). Contributions are welcome; new inference results require reproducible experiments and honest provenance. See [contribution standards](CONTRIBUTING.md). Third-party reference software retains its own licensing terms.

---

**Vers3Dynamics**  
*Resonance as Substrate. Intelligence as a Layer.*

**TRIAD does not begin by asking what the sensor sees. It begins by asking what the measurements permit us to know.**
