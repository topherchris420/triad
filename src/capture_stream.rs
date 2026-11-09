//! Causal streaming features over TRIAD capture v1, computed with the vendored
//! `flexplan` event-driven feature engine.
//!
//! Each capture frame is one event (`empty_reference` or `target_present`). A fixed,
//! validated feature graph is planned once; then every frame runs its precompiled plan
//! and the full feature vector is read back. Every feature at frame `k` depends only on
//! frames `0..=k`. That distinguishes this command from [`crate::capture::Capture::audit`],
//! whose late-minus-early drift and covariance use the whole recording.
//!
//! Features (per channel unless noted; definitions in `docs/capture-stream.md`):
//! * rolling empty-reference mean and sample standard deviation over the last `window`
//!   references (two-pass, in frame order);
//! * initial reference mean over the first `initial_reference_frames` references, and
//!   drift = rolling mean − initial mean;
//! * drift score (scalar) = max over channels of |drift| / rolling sd (0/0 counts as 0;
//!   differences within [`RESOLUTION`] of the channel magnitude count as 0);
//! * target deviation z = (target − rolling mean) / rolling sd, using reference statistics
//!   from *before* the target frame, and its maximum absolute value;
//! * milliseconds since the most recent empty reference (scalar).
//!
//! These are descriptive, causal diagnostics. They do not estimate positions, separate
//! electronic noise from drift, or validate a refusal rule. The drift score is a
//! *candidate* refusal score; whether refusing on it controls selective risk is an open
//! question that needs a pre-registered study with independently verified labels.

use crate::capture::{Capture, FrameRole};
use anyhow::{bail, ensure, Context, Result};
use flexplan::{Event, GraphBuilder, NodeHandle, RingBuffer, ValueId};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Output schema identifier.
pub const SCHEMA: &str = "triad-capture-stream-1";
const CH: usize = 10;

/// Streaming configuration, fixed before any frame is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamConfig {
    /// Number of most recent empty-reference frames in the rolling statistics.
    pub window: usize,
    /// Number of first empty-reference frames that define the initial baseline.
    pub initial_reference_frames: usize,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            window: 32,
            initial_reference_frames: 3,
        }
    }
}

impl StreamConfig {
    fn validate(&self) -> Result<()> {
        ensure!(
            (2..=100_000).contains(&self.window),
            "window must be between 2 and 100000"
        );
        ensure!(
            (1..=100_000).contains(&self.initial_reference_frames),
            "initial_reference_frames must be between 1 and 100000"
        );
        Ok(())
    }
}

/// Causal features observed immediately after one frame.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamFrame {
    pub index: usize,
    pub timestamp_ms: u64,
    pub role: FrameRole,
    /// Empty-reference frames seen so far, including this one.
    pub reference_frames_seen: u64,
    /// Empty-reference frames inside the rolling window.
    pub reference_frames_in_window: usize,
    /// Time since the most recent empty reference; `null` before the first one.
    pub ms_since_reference: Option<f64>,
    /// Rolling reference mean; `null` before the first reference.
    pub reference_mean: Option<[f64; CH]>,
    /// Rolling reference sample sd; `null` until two references are in the window.
    pub reference_stddev: Option<[f64; CH]>,
    /// Rolling mean minus initial mean; `null` until the initial baseline is complete.
    pub drift_from_initial: Option<[f64; CH]>,
    /// max over channels of |drift| / sd; `null` when drift or sd is unavailable, or when
    /// a channel drifted while its sd is 0 (0 drift with 0 sd counts as 0).
    pub drift_score: Option<f64>,
    /// Present on target frames only. A channel is `null` when its sd is unavailable, or
    /// when its sd is 0 and the target differs from the mean (equal values give 0).
    pub target_z: Option<[Option<f64>; CH]>,
    /// Maximum |z| over channels with a defined z, on target frames only.
    pub max_abs_target_z: Option<f64>,
}

/// Full streaming report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamReport {
    pub schema_version: String,
    pub capture_id: String,
    pub device_class: String,
    pub geometry_id: String,
    pub config: StreamConfig,
    pub engine: EngineInfo,
    pub frames: Vec<StreamFrame>,
    pub warning: String,
}

/// Provenance of the feature engine and its compiled plans.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineInfo {
    pub name: String,
    pub version: String,
    /// Compiled plan for each event kind, one step per line.
    pub plans: Vec<PlanInfo>,
}

/// One compiled plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanInfo {
    pub event: String,
    pub steps: Vec<String>,
}

const WARNING: &str = "Causal descriptive diagnostics only. Each feature at frame k uses frames 0..=k. Reference variation mixes electronic noise, drift and environmental effects. The drift score is an unvalidated candidate refusal score: no selective-risk control, position estimate or hardware validity is established. Raw pF readings MUST NOT enter the pF/m 2D inverse model.";

// ---------------------------------------------------------------- events

struct ReferenceFrame {
    time_ns: u64,
    channels: [f64; CH],
}
impl Event for ReferenceFrame {
    const NAME: &'static str = "empty_reference";
    fn validate(&self) -> Result<(), &'static str> {
        if self.channels.iter().all(|c| c.is_finite()) {
            Ok(())
        } else {
            Err("non-finite channel")
        }
    }
    fn time_ns(&self) -> Option<u64> {
        Some(self.time_ns)
    }
}

struct TargetFrame {
    time_ns: u64,
    channels: [f64; CH],
}
impl Event for TargetFrame {
    const NAME: &'static str = "target_present";
    fn validate(&self) -> Result<(), &'static str> {
        if self.channels.iter().all(|c| c.is_finite()) {
            Ok(())
        } else {
            Err("non-finite channel")
        }
    }
    fn time_ns(&self) -> Option<u64> {
        Some(self.time_ns)
    }
}

// ---------------------------------------------------------------- node state

struct ReferenceState {
    rings: [RingBuffer; CH],
    seen: u64,
}

struct InitialState {
    k: usize,
    count: usize,
    sums: [f64; CH],
    mean: Option<[f64; CH]>,
}

struct TargetState {
    z: [f64; CH],
}

#[derive(Default)]
struct ClockState {
    last_reference_ns: Option<u64>,
    now_ns: Option<u64>,
}

/// Relative numerical resolution: differences below `RESOLUTION * |scale|` are rounding
/// artifacts of computing means (e.g. the mean of three equal readings can differ from
/// the reading in the last bit) and are treated as exact equality.
pub const RESOLUTION: f64 = 16.0 * f64::EPSILON;

/// `x` in units of `sd`, where `scale` is the channel magnitude used for the resolution
/// floor. Deviations and spreads within the floor are exactly 0. Zero deviation with
/// zero spread is 0 (common with quantized readings); a non-zero deviation with zero
/// spread, or any non-finite input, is undefined (NaN).
fn normalized(x: f64, sd: f64, scale: f64) -> f64 {
    if !x.is_finite() || !sd.is_finite() || !scale.is_finite() || sd < 0.0 {
        return f64::NAN;
    }
    let floor = RESOLUTION * scale.abs();
    let x = if x.abs() <= floor { 0.0 } else { x };
    let sd = if sd <= floor { 0.0 } else { sd };
    if sd > 0.0 {
        x / sd
    } else if x == 0.0 {
        0.0
    } else {
        f64::NAN
    }
}

fn window_mean(r: &RingBuffer) -> f64 {
    let n = r.len();
    if n == 0 {
        return f64::NAN;
    }
    // Sequential sum in frame order: identical arithmetic to Capture::audit when the
    // window holds every reference.
    (0..n).map(|i| r.get(i).unwrap()).sum::<f64>() / n as f64
}

fn window_sd(r: &RingBuffer) -> f64 {
    let n = r.len();
    if n < 2 {
        return f64::NAN;
    }
    let m = window_mean(r);
    let var = (0..n)
        .map(|i| {
            let d = r.get(i).unwrap() - m;
            d * d
        })
        .sum::<f64>()
        / (n - 1) as f64;
    var.max(0.0).sqrt()
}

// ---------------------------------------------------------------- graph

struct Graph {
    reference: flexplan::EventKey<ReferenceFrame>,
    target: flexplan::EventKey<TargetFrame>,
    reference_node: NodeHandle<ReferenceState>,
    request: flexplan::RequestKey,
}

/// Request layout (offsets into the feature vector).
const IN_WINDOW: usize = 0;
const MEAN: usize = 1;
const SD: usize = MEAN + CH;
const DRIFT: usize = SD + CH;
const DRIFT_SCORE: usize = DRIFT + CH;
const Z: usize = DRIFT_SCORE + 1;
const MAX_Z: usize = Z + CH;
const SINCE_REF: usize = MAX_Z + 1;
const WIDTH: usize = SINCE_REF + 1;

fn build(config: StreamConfig) -> Result<(flexplan::Engine, Graph)> {
    let mut b = GraphBuilder::new();
    let reference = b.event::<ReferenceFrame>();
    let target = b.event::<TargetFrame>();

    // Rolling empty-reference statistics.
    let reference_node = b.node(
        "reference",
        ReferenceState {
            rings: std::array::from_fn(|_| RingBuffer::new(config.window)),
            seen: 0,
        },
    );
    b.on::<ReferenceFrame, _, _>(reference_node, &[], |s, f, _| {
        for (ring, &x) in s.rings.iter_mut().zip(f.channels.iter()) {
            ring.push(x);
        }
        s.seen += 1;
    });
    let in_window = b.add_named_eval(reference_node, "in_window", &[], |s, _| {
        s.rings[0].len() as f64
    });
    let mean: [ValueId; CH] = std::array::from_fn(|j| {
        b.add_named_eval(reference_node, &format!("mean{j}"), &[], move |s, _| {
            window_mean(&s.rings[j])
        })
    });
    let sd: [ValueId; CH] = std::array::from_fn(|j| {
        b.add_named_eval(reference_node, &format!("sd{j}"), &[], move |s, _| {
            window_sd(&s.rings[j])
        })
    });

    // Initial baseline: first k references, frozen afterwards.
    let initial_node = b.node(
        "initial_reference",
        InitialState {
            k: config.initial_reference_frames,
            count: 0,
            sums: [0.0; CH],
            mean: None,
        },
    );
    b.on::<ReferenceFrame, _, _>(initial_node, &[], |s, f, _| {
        if s.count < s.k {
            for (sum, &x) in s.sums.iter_mut().zip(f.channels.iter()) {
                *sum += x;
            }
            s.count += 1;
            if s.count == s.k {
                let k = s.k as f64;
                s.mean = Some(s.sums.map(|v| v / k));
            }
        }
    });
    let initial: [ValueId; CH] = std::array::from_fn(|j| {
        b.add_named_eval(initial_node, &format!("mean{j}"), &[], move |s, _| {
            s.mean.map_or(f64::NAN, |m| m[j])
        })
    });

    // Drift from the initial baseline (stateless).
    let drift_node = b.node("drift", ());
    let drift: [ValueId; CH] = std::array::from_fn(|j| {
        b.add_named_eval(
            drift_node,
            &format!("ch{j}"),
            &[mean[j], initial[j]],
            |_, r| r.get(0) - r.get(1),
        )
    });
    let score_node = b.node("drift_score", ());
    let score_reads: Vec<ValueId> = drift
        .iter()
        .chain(sd.iter())
        .chain(mean.iter())
        .copied()
        .collect();
    let drift_score = b.add_eval(score_node, &score_reads, |_, r| {
        let mut worst: f64 = 0.0;
        for j in 0..CH {
            let v = normalized(r.get(j), r.get(CH + j), r.get(2 * CH + j));
            if v.is_nan() {
                return f64::NAN;
            }
            worst = worst.max(v.abs());
        }
        worst
    });

    // Target deviation in reference-noise units, using statistics from before the frame.
    let target_node = b.node("target", TargetState { z: [f64::NAN; CH] });
    let target_reads: Vec<ValueId> = mean.iter().chain(sd.iter()).copied().collect();
    b.on::<TargetFrame, _, _>(target_node, &target_reads, |s, f, r| {
        for j in 0..CH {
            s.z[j] = normalized(f.channels[j] - r.get(j), r.get(CH + j), r.get(j));
        }
    });
    let z: [ValueId; CH] = std::array::from_fn(|j| {
        b.add_named_eval(target_node, &format!("z{j}"), &[], move |s, _| s.z[j])
    });
    let max_z = b.add_named_eval(target_node, "max_abs_z", &[], |s, _| {
        s.z.iter()
            .filter(|v| v.is_finite())
            .fold(f64::NAN, |acc, v| {
                if acc.is_nan() {
                    v.abs()
                } else {
                    acc.max(v.abs())
                }
            })
    });

    // Staleness of the reference baseline.
    let clock_node = b.node("reference_clock", ClockState::default());
    b.on::<ReferenceFrame, _, _>(clock_node, &[], |s, f, _| {
        s.last_reference_ns = Some(f.time_ns);
        s.now_ns = Some(f.time_ns);
    });
    b.on::<TargetFrame, _, _>(clock_node, &[], |s, f, _| s.now_ns = Some(f.time_ns));
    let since_ref = b.add_eval(clock_node, &[], |s, _| {
        match (s.last_reference_ns, s.now_ns) {
            (Some(r), Some(n)) => n.saturating_sub(r) as f64 / 1e6,
            _ => f64::NAN,
        }
    });

    let mut outputs = vec![in_window];
    outputs.extend(mean);
    outputs.extend(sd);
    outputs.extend(drift);
    outputs.push(drift_score);
    outputs.extend(z);
    outputs.push(max_z);
    outputs.push(since_ref);
    debug_assert_eq!(outputs.len(), WIDTH);
    let request = b.request("features", &outputs);
    let engine = b
        .build()
        .map_err(|e| anyhow::anyhow!("feature graph failed validation: {e}"))?;
    Ok((
        engine,
        Graph {
            reference,
            target,
            reference_node,
            request,
        },
    ))
}

fn finite(x: f64) -> Option<f64> {
    x.is_finite().then_some(x)
}
fn all_finite(v: &[f64]) -> Option<[f64; CH]> {
    v.iter()
        .all(|x| x.is_finite())
        .then(|| std::array::from_fn(|j| v[j]))
}

/// Stream a validated capture. Fails before reading any frame if the capture or
/// configuration is invalid.
pub fn stream(capture: &Capture, config: StreamConfig) -> Result<StreamReport> {
    capture.validate()?;
    config.validate()?;
    let (mut engine, g) = build(config)?;
    let plans = ["empty_reference", "target_present"]
        .iter()
        .map(|&e| PlanInfo {
            event: e.to_string(),
            steps: engine
                .describe_event_plan(e)
                .unwrap_or_default()
                .lines()
                .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
                .collect(),
        })
        .collect();
    let mut out = vec![0.0; WIDTH];
    let mut frames = Vec::with_capacity(capture.frames.len());
    for (index, f) in capture.frames.iter().enumerate() {
        let time_ns = f
            .timestamp_ms
            .checked_mul(1_000_000)
            .context("timestamp_ms too large to express in nanoseconds")?;
        let dispatched = match f.role {
            FrameRole::EmptyReference => engine.dispatch(
                g.reference,
                &ReferenceFrame {
                    time_ns,
                    channels: f.channels,
                },
            ),
            FrameRole::TargetPresent => engine.dispatch(
                g.target,
                &TargetFrame {
                    time_ns,
                    channels: f.channels,
                },
            ),
        };
        dispatched.with_context(|| format!("frame {index} refused"))?;
        engine
            .request(g.request, &mut out)
            .with_context(|| format!("feature request after frame {index} refused"))?;
        let is_target = matches!(f.role, FrameRole::TargetPresent);
        frames.push(StreamFrame {
            index,
            timestamp_ms: f.timestamp_ms,
            role: f.role,
            reference_frames_seen: engine.state(g.reference_node).seen,
            reference_frames_in_window: out[IN_WINDOW] as usize,
            ms_since_reference: finite(out[SINCE_REF]),
            reference_mean: all_finite(&out[MEAN..MEAN + CH]),
            reference_stddev: all_finite(&out[SD..SD + CH]),
            drift_from_initial: all_finite(&out[DRIFT..DRIFT + CH]),
            drift_score: finite(out[DRIFT_SCORE]),
            target_z: is_target.then(|| std::array::from_fn(|j| finite(out[Z + j]))),
            max_abs_target_z: if is_target { finite(out[MAX_Z]) } else { None },
        });
    }
    Ok(StreamReport {
        schema_version: SCHEMA.into(),
        capture_id: capture.capture_id.clone(),
        device_class: capture.device_class.clone(),
        geometry_id: capture.geometry_id.clone(),
        config,
        engine: EngineInfo {
            name: "flexplan".into(),
            version: "0.2.0".into(),
            plans,
        },
        frames,
        warning: WARNING.into(),
    })
}

/// Read a capture file (same 50 MB limit as `inspect-capture`) and stream it.
pub fn stream_file(path: &Path, config: StreamConfig) -> Result<StreamReport> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    ensure!(
        bytes.len() <= 50_000_000,
        "capture exceeds 50 MB inspection limit"
    );
    let capture: Capture = serde_json::from_slice(&bytes).context("parse TRIAD capture v1 JSON")?;
    stream(&capture, config)
}

const HELP: &str = "triad stream-capture --input FILE [--window N] [--initial-reference-frames N]

Replay a TRIAD capture v1 as an event stream and print causal per-frame features
(JSON, schema triad-capture-stream-1) to stdout. Every feature at frame k uses
frames 0..=k only. Defaults: --window 32, --initial-reference-frames 3.

Descriptive diagnostics only; see docs/capture-stream.md for definitions and limits.";

/// Command-line entry point (`args` excludes the subcommand name).
pub fn cli(args: &[String]) -> Result<()> {
    let mut input: Option<PathBuf> = None;
    let mut config = StreamConfig::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = |name: &str| -> Result<String> {
            it.next()
                .cloned()
                .with_context(|| format!("{name} requires a value"))
        };
        match a.as_str() {
            "--help" | "-h" => {
                println!("{HELP}");
                return Ok(());
            }
            "--input" => input = Some(PathBuf::from(value("--input")?)),
            "--window" => {
                config.window = value("--window")?
                    .parse()
                    .context("--window must be an integer")?
            }
            "--initial-reference-frames" => {
                config.initial_reference_frames = value("--initial-reference-frames")?
                    .parse()
                    .context("--initial-reference-frames must be an integer")?
            }
            other => bail!("unexpected argument '{other}'; use --help"),
        }
    }
    let path = input.context("stream-capture requires --input FILE")?;
    let report = stream_file(&path, config)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
