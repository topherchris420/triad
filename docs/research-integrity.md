# Research integrity: evidence survives the run

TRIAD separates the recorded baseline from new scientific work. These safeguards protect evidence and make mistakes visible; they do not prove calibration or establish the hypothesis.

## Before computing

`selective` compares every executable setting in `experiments/selective-v3.json` against the implemented study: sequence counts, all seeds, FEM spacing, calibration frames, posterior temperature, score weight, bootstrap settings and the decision rule. A mismatch is an error before test generation. Descriptive prose is not an executable configuration API. Scientific changes require a new versioned protocol and output path, with independently seeded data when required.

The command checks the baseline validation/configuration hash and numerical-audit identity before reserving its output directory. The numerical readiness flag may be false: exploratory execution is still permitted, but the report cannot mark the hypothesis supported on that basis.

## During and after computing

1. Exclusive directory creation reserves `results/selective-v3/`. Concurrent or repeated runs cannot reuse it.
2. `run.json` records protocol, baseline configuration, dictionary identity and numerical-audit hash.
3. `test-scores.json` is saved before test truth is joined into the evaluation.
4. `evaluation.json` records errors only after predictions have been saved.
5. `summary.json` is written last, with SHA-256 hashes of the run, score and evaluation files. Its presence is the completion marker; parse and hash checks detect a truncated or inconsistent bundle.

All new research JSON files use exclusive creation and file synchronization. These are append-only safeguards, not cryptographic signatures or a guarantee against deliberate manual alteration. Baseline reproduction commands retain their original behavior and may replace baseline outputs.

Geometry screening verifies the selective protocol and artifact hashes before using that completed study as its prerequisite. A hash-consistent study can still have a negative or inconclusive scientific result.

## Interrupted runs

If the directory exists without a valid summary, preserve it with its logs. A rerun fails rather than replacing predictions or exposed test outcomes. There is no automatic resume or `--force` mode. Archive the incomplete directory outside the active output path before any approved recovery; once test information has been exposed, use a new study version and fresh test seeds rather than treating a retry as untouched evidence.

For isolated development, copy the repository/data tree and use `triad --root /path/to/copy COMMAND`. This changes the artifact location, not the statistical independence of reused data or seeds.

## Verification boundary

CI runs the Rust tests and derived baseline audits, then checks that the selected frozen evidence paths are unchanged. Tests cover protocol mutation/omission, output replacement, CLI failures, numerical behavior and statistical diagnostics. CI deliberately does not execute the expensive final study or turn software-test success into a scientific finding.
