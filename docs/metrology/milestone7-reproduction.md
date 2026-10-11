# Milestone 7 reproduction package

Written for a researcher who has never used TRIAD. It needs no private files,
credentials or undocumented settings, only public network access to crates.io,
PyPI and the UCI repository.

## 1. Install

| Requirement | Recorded version | Notes |
|---|---|---|
| Git | any recent | clone with default settings; `.gitattributes` forces LF checkouts |
| Rust toolchain | rustc/cargo 1.97.0 (recorded host); CI uses current stable | `--locked` pins every crate in `Cargo.lock`; a C toolchain is needed for native dependencies |
| Python | 3.13.16 (recorded host); CI uses 3.12 | 3.11–3.13 have wheels for the pinned packages |
| Python packages | `benchmarks/bike-sharing/requirements.txt` | numpy 2.2.6, scipy 1.15.3, scikit-learn 1.6.1, matplotlib 3.10.3, tzdata 2025.2 (needed on Windows for `zoneinfo`) |

```sh
git clone https://github.com/topherchris420/triad.git
cd triad
python -m venv .venv && . .venv/bin/activate      # Windows: .venv\Scripts\activate
python -m pip install -r benchmarks/bike-sharing/requirements.txt
cargo build --release --locked
cargo test --release --locked
```

## 2. Retrieve and audit the dataset

```sh
python scripts/audit-bike-sharing.py --cache work/bike-cache --output work/bike-data-audit.json
```

This downloads `https://archive.ics.uci.edu/static/public/275/bike+sharing+dataset.zip`
and the UCI metadata API response. It refuses to continue if `hour.csv` does not
have SHA-256 `e03de4ee4ef4dc376ac6e04bf829673c6269e8eba5c60fa121640fa2f829504f`.
Except for the live `source` block, the output must equal
`benchmarks/bike-sharing/data-audit.json`. If UCI re-packages the archive, the zip
hash may differ while `hour.csv` still matches; the audit reports this.

## 3. Reproduce the measurements (Rust)

```sh
cargo run --release --locked -- metrology benchmark --input benchmarks/bike-sharing/manifest.json --output results/my-bike-sharing
cargo run --release --locked -- metrology validate --input results/my-bike-sharing/report.json
cmp results/my-bike-sharing/report.json results/bike-sharing-v7/report.json

cargo run --release --locked -- metrology evidence-suite --input benchmarks/evidence-validation/suite.json --output results/my-evidence-suite
cargo run --release --locked -- metrology validate --input results/my-evidence-suite/report.json

cargo run --release --locked -- metrology evidence --input examples/evidence-assessment.json --output results/my-evidence-assessment.json
cargo run --release --locked -- metrology validate --input results/my-evidence-assessment.json
```

Run these from the repository root: evidence `file_hash` paths resolve against
`--root` (default: the current directory). Output paths must be new; nothing is
overwritten.

**Expected outputs.** `report.md` must show ridge test MAE 32.84 bikes,
material-error rate 0.0969, primary Δ +0.00490 [+0.00059, +0.01052]; HGB MAE 51.18,
0.2103, +0.00624 [+0.00142, +0.01224]; both "evidence insufficient to establish a
practical advantage". The suite must show TRIAD 43/47 detected, 0/16 false
rejections; C 20/47 and 0/16. On the recorded Linux host the bike report is
byte-identical to the archive. **Tolerance:** if your platform's floating point
differs, `metrology validate` on your own output must still pass. Compare numbers
with the archive at relative 1e-9. Classifications must match exactly.

## 4. Independent computational verification (Python)

```sh
python scripts/verify-milestone7.py --bike results/my-bike-sharing/report.json \
  --suite results/my-evidence-suite/report.json \
  --evidence results/my-evidence-assessment.json \
  --data work/bike-cache --plots results/my-plots
```

Expected: `PASS: 3027 independent checks … and 102 frozen pre-M7 artifacts.`
Tolerance: relative 1e-9, absolute 1e-11. The script never imports Rust. It
recomputes elapsed time with `zoneinfo`, KS with SciPy, Clopper–Pearson with
`scipy.stats.beta`, and re-implements every evidence rule from
[the method](evidence-sufficiency-method.md).

## 5. Retrain the frozen models from source data

```sh
python scripts/prepare-bike-sharing.py --cache work/bike-cache --output work/bike-retrained
python scripts/check-bike-reproduction.py work/bike-retrained
```

The second command must print `PASS`. The tolerance is relative 1e-8 and absolute
1e-9 on predictions, error scales, probabilities, coefficients and tree
thresholds. Each run's own model-file hashes are verified. Byte hashes may differ
across platforms because of BLAS and compiler differences; on the recorded host,
two clean runs were byte-identical.

## 6. Hashes

| Artifact | SHA-256 |
|---|---|
| `hour.csv` (inside the UCI archive) | `e03de4ee4ef4dc376ac6e04bf829673c6269e8eba5c60fa121640fa2f829504f` |
| UCI archive observed 2026-10-11 | `b70182d0d0508e9abbb79306ce5c0cec34869000f8220175ac83d11dbe845401` |
| `benchmarks/bike-sharing/protocol.json` | `c4d491fade7debe334b092806bff7c180af49b370759048f56686d985d6bd1c2` |
| Archived reports | listed in [`results/milestone7-verification.json`](../../results/milestone7-verification.json) |
| Pre-Milestone-7 artifacts (102) | [`benchmarks/bike-sharing/frozen-before-v7.json`](../../benchmarks/bike-sharing/frozen-before-v7.json) |

## 7. Known platform differences

- Python `zoneinfo` on Windows needs the pinned `tzdata` package.
- HistGradientBoosting is run single-threaded so that summation order does not
  depend on core count; scikit-learn wheels compiled on different platforms may
  still differ at round-off level. The reproduction check allows for this.
- The Rust evidence engine computes beta quantiles with platform `ln`/`exp`.
  Reports store only classifications and six-decimal reason text, so last-bit
  differences do not change them.

## 8. Limitations that reproduction does not remove

Computational reproducibility shows that the code computes what it says. It does
not validate the tolerance, the abstention cost, the claim-suite ground truth (one
author), or generalisation beyond one city and one autumn. See
[results §10](milestone7-results.md#10-limitations).

## 9. Reviewer checklist

- [ ] The protocol commit `bb1bd0f` precedes the prediction commit `9c63076`, which
      precedes every result file (`git log --follow` on each path).
- [ ] `protocol.json` hash equals `manifest.protocol_sha256`; models hash to
      `model_sha256`.
- [ ] No 2012 target enters any feature, scaler, threshold or rule
      (`scripts/prepare-bike-sharing.py`; Rust test
      `test_period_counts_cannot_move_fitted_quantities`).
- [ ] Lags use elapsed hours across gaps and DST; ineligible targets are excluded,
      not imputed.
- [ ] Comparators were matched on calibration alert budget, and the test budget
      differences are reported.
- [ ] Every comparator and ablation in `report.json` is reported in the results
      document, favourable or not.
- [ ] Suite ground truth is defensible for each case. Disagree in an issue: the
      ground truth was written by the same author as the rules.
- [ ] Misleading-input failures are counted, not excluded.
- [ ] Nothing in the README or NIST draft claims more than the results support.

To report a problem or disagreement, open an issue with the
[scientific criticism template](../../.github/ISSUE_TEMPLATE/scientific-criticism.md).
No unrelated-investigator replication has yet been performed.
