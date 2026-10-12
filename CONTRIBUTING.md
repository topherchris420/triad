# Contributing to TRIAD

TRIAD is a research instrument, not a place to accumulate features without experiments.

## Scientific bar

- Keep proposed hypotheses separate from implemented algorithms and measured findings.
- Do not rewrite frozen test artifacts to make results favorable. Add a versioned protocol and *new independent test sequences* when modifying an estimator or evaluation rule.
- Use documented SI units and distinguish pF/m in the idealized 2D model from capacitance in pF for physical hardware.
- Make any new numerical approximation accountable against mesh refinement, known analytical solutions, and the measurement noise model.
- Avoid ground-truth leakage into inference-time observability or rejection scores.
- Benchmark against strong alternatives, especially the posterior expected-error selector, and report negative findings.
- Record random seeds, configuration hashes, source versions, and full reproducibility commands.

## Software quality

- Keep the Rust package lean. Add tests for new statistical and numerical behavior.
- Run `cargo fmt --all -- --check` and `cargo test --locked` and, when changing physics, the validated simulation on development inputs.
- Avoid making the full expensive FEM rerun a requirement for every code change.
- Treat archived artifacts as evidence, not scratch files.
- When README.md quotes a result, bind it in [docs/claims-ledger.json](docs/claims-ledger.json) and run `python scripts/check-claims.py`. Edit the ledger, not the generated findings table; `--write` re-renders it.

## Third-party rights

[Infineon CAPSENSE](https://github.com/Infineon/capsense) is a proprietary industrial reference. Reading its public source does not grant blanket rights to redistribute code or proprietary derivatives. REA analysis must be authorized and lawful. Do not submit copied or decompiled vendor code as a TRIAD contribution.

**Project license:** [MIT](LICENSE), copyright Christopher Woodyard, Vers3Dynamics. This covers TRIAD source; external references retain their own licenses.

Before changing study code, read [research integrity](docs/research-integrity.md). Preserve interrupted runs and frozen artifacts. Version scientific changes explicitly; do not alter seeds or decision rules after seeing test outcomes.

Principal investigator: Christopher Woodyard, Vers3Dynamics.
