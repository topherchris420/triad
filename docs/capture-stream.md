# Causal capture-stream features

`triad stream-capture` replays a [capture v1](portable-adoption.md#b-physical-capture-interchange-and-noise-inspection) file as an ordered event stream. It reports, for every frame, features computed **only from that frame and earlier ones**.

```sh
cargo run --locked -- stream-capture --input examples/synthetic-capture.json
cargo run --locked -- stream-capture --input FILE --window 32 --initial-reference-frames 3
python scripts/verify-capture-stream.py CAPTURE.json STREAM.json
```

**Status:** an engineering component, not a research finding. It adds no claim to the README findings and changes no frozen evidence. The committed capture is a synthetic fixture.

## Why a causal view

`inspect-capture` summarises a whole recording at once. Its covariance and its late-minus-early drift use every empty-reference frame, including frames recorded *after* any given moment. That is appropriate for describing a finished session. It is not what a deployed system could know at the time it must decide whether to answer.

`stream-capture` gives the deployable counterpart: at frame *k* it sees frames 0…*k* only. A test (`features_never_depend_on_future_frames`) truncates the capture, and separately rewrites every later frame. In both cases it checks that frames 0…*k* are unchanged.

## Features

Let *R(k)* be the most recent `window` empty-reference frames up to frame *k*, and *I* the first `initial_reference_frames` empty references. Statistics are per channel *j*, in the capture's declared unit.

| Field | Definition | `null` when |
|---|---|---|
| `reference_frames_seen` | empty references so far, including this frame | — |
| `reference_frames_in_window` | \|R(k)\| | — |
| `ms_since_reference` | timestamp − timestamp of the latest empty reference | no reference yet |
| `reference_mean` | mean of R(k), summed in frame order | no reference yet |
| `reference_stddev` | sample sd of R(k), two-pass, divisor \|R(k)\| − 1 | fewer than 2 references in the window |
| `drift_from_initial` | `reference_mean` − mean of I | I not yet complete |
| `drift_score` | max over channels of \|drift\| / sd, using the normalization rule below | drift or sd unavailable, or a channel drifted while its sd is 0 |
| `target_z` | target frames only: (frame − `reference_mean`) / `reference_stddev`, with statistics from **before** the frame | per channel, under the normalization rule |
| `max_abs_target_z` | target frames only: largest defined \|z\| | no channel defined |

Target frames never update reference statistics.

**Normalization rule.** Deviations and spreads below `RESOLUTION × |channel mean|` are rounding artifacts and count as exactly 0; `RESOLUTION` is 16 machine epsilons. For example, the mean of three equal readings can differ from the reading in its last bit. After that floor is applied:

| Deviation | Spread (sd) | Result |
|---|---|---|
| any | > 0 | deviation / sd |
| 0 | 0 | 0 (common with quantized readings) |
| non-zero | 0 | undefined (`null`), never infinite |

Reported means and sds are **raw**, not floored.

**Exactness check.** With a window covering every reference, the final `reference_mean` and `reference_stddev` equal `inspect-capture`'s `empty_mean` and `empty_sample_stddev` **bit for bit** (`final_rolling_statistics_equal_the_batch_audit_exactly`). They use the same formula and summation order.

## Engine

The features run on [flexplan](../crates/flexplan/VENDORED.md), a dependency-free event-driven feature engine vendored under `crates/flexplan`. It is an independent clean-room implementation of ideas from Jane Street's public *Flex Model* article, not Jane Street code.

Each feature is a node with event handlers (which update state) and evals (which compute values on demand). A planner compiles, at startup, an ordered step list per event kind; the report includes both plans. On an `empty_reference` frame only the three state handlers run. On a `target_present` frame the reference statistics are evaluated first, then the target handler reads them. Each frame's plan runs with no change-cutoffs, so repeated identical frames each count.

Input checks happen before any state changes. The capture is validated by `Capture::validate`: units, channel order, strictly increasing timestamps, finite channels. Each frame is also checked by the engine's event validation and its monotonic clock.

For ten channels the engine's speed is irrelevant. It is used for its guarantees: declared dependencies, deterministic ordering, causal evaluation, and per-event plans that can be inspected.

## Verification

| Check | Where |
|---|---|
| 11 Rust tests: exact audit agreement, no future leakage, repeated identical frames, window wraparound, initial-baseline drift, pre-frame target statistics, null-not-zero evidence gaps, staleness, zero-noise channels, invalid input, determinism and schema round-trip | `tests/capture_stream.rs` |
| Independent standard-library Python recomputation of every field, including null agreement | `scripts/verify-capture-stream.py`; runs in CI on the committed fixture |

## What this does not establish

* **No refusal rule is validated.** `drift_score` is a *candidate* refusal score (smaller is safer, matching TRIAD's convention). Whether refusing on it controls selective risk is an empirical question. It needs a pre-registered protocol, captures from independent sessions and devices, verified bad-event labels that the score never sees, and comparison with stronger selectors, as described in [CONTRIBUTING](../CONTRIBUTING.md).
* **No noise model.** Reference variation mixes electronic noise, drift, temperature and environment. A rolling sd is not a calibrated measurement uncertainty.
* **No positions, no hardware validity.** Raw pF captures must not enter the pF/m 2D inverse model. The physical-model limitations in [limitations.md](limitations.md) are unchanged.

## Connecting to TRIAD's decision tools

The per-frame `drift_score` (or `max_abs_target_z`) has the shape that `calibrate-abstention` and `certify-abstention` expect for a frame score. Producing their inputs additionally requires, per independent sequence, a bad-event label for each frame, declared and verified independently of the score. This command deliberately does not create or guess labels.
