# Reproduction

Rust stable with the locked dependencies; a platform C toolchain is required by
transitive native dependencies. Build and test before interpreting any output:

```sh
cargo fmt --all -- --check
cargo test --locked
cargo clippy --locked --all-targets
cargo run --release --locked -- metrology --help
cargo run --release --locked -- metrology evaluate --input examples/predictions.json --output results/metrology-example.json
cargo run --release --locked -- metrology validate --input results/metrology-example.json
cargo run --release --locked -- metrology demo --output results/metrology-demo
cargo run --release --locked -- metrology plan --input results/metrology-demo/predictions.json --output results/metrology-planning.json
```

Use new paths if committed outputs already exist; outputs are append-only.
The JSON schema is `input-schema.json`; semantic checks additionally enforce
dimensions, split/cluster separation, valid losses and nonempty calibration/test.
Euclidean loss uses the supplied vector units; capacitive XY coordinates are
meters and a 5 mm criterion is tolerance 0.005. Classification is scalar integer
class mismatch with tolerance below 1. Smaller uncertainty means more confidence.

Each report embeds the complete input and a canonical serialization SHA-256,
package version, evaluator source and dependency-lock hashes, original input byte hash, runtime platform and measured elapsed
seconds. `validate` recomputes deterministic report contents, including seeded
bootstrap values. Execution time is not deterministic or a scientific endpoint.
It does not authenticate external evidence or externally trained model identity.

The demo trains logistic regression on 800 independently seeded rows, fixes
weights before a separate 400-row development pilot and 400-row calibration, then evaluates 600 unshifted and 600 shifted rows.
Model coefficients, seeds, training-data hash and optimization settings are saved.
The concept shift reverses feature coefficients while retaining the learned model.
No FEM is involved. `scripts/verify-metrology.py` independently checks metrics
using Python's standard library (not the Rust evaluator).

Expensive numerical study, separate from the fast demo:

```sh
cargo run --release --locked -- metrology numerical --output results/metrology-numerical-v5
```

Eight probe/layout combinations each require three mesh solutions plus expanded
boundary, dielectric-drift and edge/interface-refined solutions. Every solution includes a homogeneous
reference solve. `started.json` without `summary.json` denotes an incomplete run,
never a completed study. Three-grid agreement is not proof of physical truth.

Historical byte hashes are in `baseline-hashes.json` and tested by Rust. On
platforms that convert Git text line endings, clone with `core.autocrlf=false`
to preserve archived artifact bytes. Floating-point metrics may vary at rounding
level between platforms; record the actual platform rather than claiming universal
bitwise reproducibility.

NIST draft validation (requires Python `jsonschema` and `pyyaml`): clone the
official submissions repository, select the recorded schema revision, and run
its documented validation command. The placeholder contact must fail; do not
replace it with an invented address. Revalidate against current main before any
future manual submission.

Bounded inference comparison (two new development cases):

```sh
cargo run --release --locked -- metrology adaptive --output results/metrology-adaptive-v5
```

The direct refined comparator uses the same measurement-selected cell shortlist,
not a global fine-FEM optimizer. Both grids sample up to 54 candidates and retain
all residuals; physical confidence is withheld.

For the committed demonstration, validate
`results/metrology-demo-v5-canonical/report.json`. The earlier local Windows
snapshot records different source line endings and is retained for provenance;
its source hash will not match a canonical Git checkout. Newly generated demos
from a Git checkout use the canonical source bytes.
