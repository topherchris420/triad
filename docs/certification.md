# Certifying a refusal policy with less data

**Status: statistical engineering, checked by a pre-registered simulation
(Milestone 9). It is not a sensor, hardware or real-data result.** The methods
are established; TRIAD supplies an implementation, a proof that it dominates
TRIAD's earlier controller, and an archived, independently verified study.

## The question

A refusal policy accepts a prediction when its risk score is at most a threshold
`t`. Before deploying it, you want a statement of the form

> With probability at least `1 − δ` over the calibration data, the accepted
> predictions are wrong at most `α` of the time, and at least `c` of inputs are
> accepted.

TRIAD's existing controller ([`src/abstention.rs`](../src/abstention.rs),
documented in [portable adoption](portable-adoption.md#a-conservative-abstention-on-independent-sequences))
makes that statement correctly, but it is very expensive. In the Milestone 9
simulation it needed **25,000 independent labeled sessions** to certify reliably
at the targets below. Few teams can collect that many before deployment. A
guarantee that is too expensive to obtain is rarely used.

## What changes

Every candidate threshold is still tested at `δ/(2K)` for risk and for coverage.
This is the legacy Bonferroni budget over a predeclared set of `K` candidates.
Three established ideas replace the ratio of two Hoeffding bounds.

**1. Linearise the risk.** For a fixed threshold, conditional risk is
`R(t) = J(t) / C(t)`. Here `C` is coverage and `J` is the joint rate of accepted
errors. Because `C > 0`,

```
R(t) ≤ α   ⇔   J(t) − α·C(t) ≤ 0.
```

For each independent sequence, let `a` be its accepted fraction and `e` its
accepted-incorrect fraction. Then `Z = e − α·a + α` lies in `[0, 1]`, and its
mean is at most `α` exactly when the risk claim holds. One bounded-mean test
replaces a ratio of two separately widened bounds. Frames inside a sequence may
be dependent; the sequence is the independent unit.

**2. Use a tail bound that knows the risk is small.** The Hoeffding–Bentkus
p-value (Bates, Angelopoulos, Lei, Malik and Jordan, 2021) for `H₀: E[Z] ≥ α` is

```
p = min( exp(−n · kl(min(Z̄, α) ‖ α)),  e · P(Binomial(n, α) ≤ ⌈n Z̄⌉) ).
```

`kl` is the Bernoulli Kullback–Leibler divergence. Coverage uses the matching
Chernoff–KL lower confidence bound. These are the bounds used by risk-controlling
prediction sets and Learn then Test (Angelopoulos, Bates, Candès, Jordan and
Lei, 2021).

**3. Optionally, bet.** The `betting` method replaces the tail bounds with
predictable-plug-in capital processes (Waudby-Smith and Ramdas, 2024). Ville's
inequality turns `1 / max capital` into a valid p-value, and the bets adapt to
the observed sequence-level variance. Sequence order must not depend on the
data, so TRIAD orders sequences by the SHA-256 of their identifiers.

## Dominance, proved

**Claim.** For the same data, targets and candidates, `hoeffding-bentkus`
qualifies every candidate that the legacy controller qualifies. Its coverage
lower bound is never smaller.

*Coverage.* Legacy reports `Ĉ − r`, with `r = √(ln(2K/δ)/(2n))`. Pinsker's
inequality gives `kl(Ĉ ‖ Ĉ − r) ≥ 2r² = ln(2K/δ)/n`. Since `kl(Ĉ ‖ c)` decreases
in `c` below `Ĉ`, the Chernoff–KL bound is at least `Ĉ − r`.

*Risk.* For `α < 1`, legacy qualifies only if `Ĉ − r > 0` and
`(Ĵ + r)/(Ĉ − r) ≤ α`. This means `αĈ − Ĵ ≥ r(1 + α) ≥ r`. Then the gap between
`α` and `Z̄ = Ĵ − αĈ + α` is at least `r`. By Pinsker again,
`exp(−n·kl(Z̄ ‖ α)) ≤ exp(−2nr²) = δ/(2K)`, so the risk test rejects. For
`α = 1` the risk claim is trivially true. ∎

The implementation also accepts the legacy condition directly as a sufficient
condition for rejection. This guarantees dominance in floating point and costs
nothing in validity, since the condition implies the exact rejection.
`tests/certify.rs` checks dominance on 300 random studies, and the Milestone 9
study checked it on 32,000 simulated datasets.

The betting method has **no** dominance guarantee. It is usually much stronger,
but it is a different test. Choose a method before seeing calibration data.
Running both and keeping whichever certifies spends the error budget twice.

## Use it

The input is the same JSON as `calibrate-abstention`:

```sh
cargo run --release --locked -- certify-abstention --input examples/abstention-calibration-demo.json
cargo run --release --locked -- certify-abstention --method betting --input YOUR_REQUEST.json
```

The four-sequence demo is refused, as it should be. On the clean three-frame
fixture used by the abstention tests, the legacy controller first certifies at
**161** sequences and `hoeffding-bentkus` at **29**.

The certificate lists every candidate with its observed coverage, conditional
error, p-values, coverage bound and decision. `Certificate::decide` fails closed
exactly like `FittedPolicy::decide`:

```rust
use triad::{abstention::Decision, certify::{certify, Method}};
let cert = certify(&request.spec, &request.sequences, Method::HoeffdingBentkus)?;
let show = cert.decide(Some(score)) == Decision::Accept;
```

### How many sessions should I collect?

Plan before you collect data:

```sh
cargo run --release --locked -- plan-certification --coverage 0.6 --risk 0.026 \
    --alpha 0.05 --min-coverage 0.5 --delta 0.05 --candidates 20
```

The planner reports the number of sequences at which a study would certify if
its observed coverage and error matched your anticipated values: here **17,771**
for legacy and **2,767** for Hoeffding–Bentkus. Treat these figures as plug-in
planning numbers, not power calculations. About half of studies at exactly that
size fall short, so plan with margin. `null` means the anticipated operating
point cannot be certified at any size. The betting method has no closed form; it
does better when per-session outcomes vary little.

## Milestone 9: the pre-registered simulation

The [protocol](../experiments/certify-v1.json) was committed before any
simulation code existed. The implementation and the
[independent verifier](../scripts/verify-certify.py) were committed before the
study ran once. Scores are uniform, so true coverage is `C(t) = t`.

| Setting | Values |
|---|---|
| Targets | α = 0.05, minimum coverage 0.5, δ = 0.05, K = 20 thresholds (0.05 to 1.00) |
| Graded error model | `q(s) = 0.01 + 0.3 s³`, so `R(t) = 0.01 + 0.075 t³` |
| Null error model | `q = 0.055 > α` everywhere, so every certification is false |
| Regimes | 1 frame per sequence; or 10 frames per sequence sharing an error multiplier `2u`, `u ~ U(0,1)` |
| Grid | 100 to 25,000 sequences, 1,000 replicates per cell, 32 cells |

Results: [`report.md`](../results/certify-v1/report.md) and
[`report.json`](../results/certify-v1/report.json).

| Pre-registered hypothesis | Result |
|---|---|
| H1: no valid method exceeds δ false certification in any cell | **Supported.** No exceedance in any of the 32 cells |
| H2: Hoeffding–Bentkus dominates legacy on every dataset | **Supported.** 0 violations |
| H3: Hoeffding–Bentkus reaches 80% certification with at most half the legacy sequences (graded, dependent frames) | **Supported.** 2,500 versus 25,000 |
| H4: the same criterion for betting | **Supported.** 1,000 versus 25,000 |
| E1 (exploratory): the naive frame-level test over-certifies under dependence | **Not confirmed.** Naive false certification stayed at or below 0.001 |

At 25,000 dependent sessions, all three valid methods certify every time, but
not at the same operating point. The mean true coverage of the selected threshold
is **0.65** for legacy, **0.75** for Hoeffding–Bentkus and **0.77** for betting.
From the same data, the sharper certificate permits more answers at the same
guaranteed risk.

### What did not go as expected

The naive comparator treats every frame as independent. That assumption is false
in the dependent regime, so the expected outcome was over-certification in the
null model. **It did not over-certify.** The null error rate (0.055) sits only
10% above α, and the per-test budget δ/(2K) = 0.00125 is strict. Under that
design, the moderate within-sequence clustering did not cause measurable
over-certification. With 250 dependent sessions the naive test certified 98% of
graded studies, where the valid methods could not certify at all. Its guarantee
is still invalid under dependence. This study does not show that the
invalidity matters in practice, and it does not show that it is harmless. A
future protocol should probe stronger clustering, nulls closer to α and looser
δ before claiming either.

## Limits

- Results are simulations under the methods' own assumptions: independent,
  identically distributed sequences, analytic truth, fixed candidates and
  correct labels. Real deployments violate these through drift, site and device
  shift, label noise and selection.
- The guarantee is marginal over calibration draws and population-level. It
  says nothing about any individual prediction or about a different population.
- The efficiency gain depends on the error rate being small relative to the
  Bernoulli worst case. At larger α the advantage shrinks.
- Bonferroni over `2K` tests is simple but conservative. Fixed-sequence or
  graph-based testing (as in Learn then Test) could do better when the candidate
  ordering is predeclared.
- No real dataset has been certified with these methods yet. That requires a
  new frozen protocol and fresh independent sessions.

## Verification

```sh
cargo test --release --locked --test certify
python scripts/verify-certify.py --triad target/release/triad --random 60
cargo run --release --locked -- certify-study --validate results/certify-v1/report.json
python scripts/verify-certify.py --study results/certify-v1/report.json
```

The Python verifier recomputes every certificate with standard-library code
written separately from Rust: lgamma binomial sums, its own bisection and its
own SHA-256 ordering. It also re-derives every Clopper–Pearson bound and verdict
of the study from cell counts. `--validate` reruns the full simulation and
requires an exact match. This is independent code, not independent
investigators.

## References

- Bates, Angelopoulos, Lei, Malik and Jordan (2021). Distribution-free,
  risk-controlling prediction sets. *Journal of the ACM* 68(6).
  [arXiv:2101.02703](https://arxiv.org/abs/2101.02703)
- Angelopoulos, Bates, Candès, Jordan and Lei (2021). Learn then Test:
  calibrating predictive algorithms to achieve risk control.
  [arXiv:2110.01052](https://arxiv.org/abs/2110.01052)
- Waudby-Smith and Ramdas (2024). Estimating means of bounded random variables
  by betting. *JRSS-B* 86(1). [arXiv:2010.09686](https://arxiv.org/abs/2010.09686)
- Bentkus (2004). On Hoeffding's inequalities. *Annals of Probability* 32(2).
- Geifman and El-Yaniv (2017). Selective classification for deep neural
  networks. [arXiv:1705.08500](https://arxiv.org/abs/1705.08500)
