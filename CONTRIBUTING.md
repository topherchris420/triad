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
- Run `cargo test --locked` and, when changing physics, the validated simulation on development inputs.
- Avoid making the full expensive FEM rerun a requirement for every code change.
- Treat archived artifacts as evidence, not scratch files.

## Third-party rights

[Infineon CAPSENSE](https://github.com/Infineon/capsense) is a proprietary industrial reference. Reading its public source does not grant blanket rights to redistribute code or proprietary derivatives. REA analysis must be authorized and lawful. Do not submit copied or decompiled vendor code as a TRIAD contribution.

**Project license:** Not yet selected. Until an appropriate license is added, do not infer permission to reuse TRIAD source simply from repository access. Contact the maintainer about contribution and redistribution permissions before submitting substantive code.

Principal investigator: Christopher Woodyard, Vers3Dynamics.
