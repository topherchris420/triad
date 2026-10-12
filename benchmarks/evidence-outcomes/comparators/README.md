# Blind comparators R and S

`m8_comparators.py` and `test_m8_comparators.py` were written by a separately
briefed agent: an AI subagent of the same model family as the agent that designed
the experiment. It worked in an isolated scratch directory whose only input was
[`../comparator-brief.md`](../comparator-brief.md).

- It was instructed not to read the TRIAD repository, its rules, method document,
  Milestone 7 suite, source, history or any result, and not to search the web for
  the project. In its hand-back it listed the paths it read: the brief, and a
  listing of its own directory. Isolation is instruction-based and self-reported,
  not technically enforced.
- It received no real data. Its 42 tests use synthetic assessments it built.
- The files are committed **byte-identical** to the delivered versions
  (SHA-256 `b989e9e4…117d` and `190b5e43…9108`). They were frozen in Git before
  any real Milestone 8 case was generated, and are not modified afterwards.
- Case identifiers are used only to seed resampling.

Decision rules are stated in the module docstring. In short:

- **R** is a REFORMS-style checklist. It applies exact-scope rules, requires at
  least 30 units, and accepts only uncertainty that matches how the data were
  collected: a design effect, or a block or cluster bootstrap, for dependent data.
  It accepts coverage generalisation when the shift diagnostic is within its
  threshold, and requires every monitoring item for operational claims.
- **S** is an applied statistician. It estimates design effects from raw outcomes
  by batch means, falls back to √n effective units when dependence is unresolvable,
  computes Beta-distribution conformal coverage with a PSI allowance, and uses
  beta-binomial predictive bounds for next-week claims.
