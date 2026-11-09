use triad::capture::{Capture, CaptureFrame, CaptureSpace, CaptureUnit, FrameRole};
use triad::capture_stream::{stream, StreamConfig, StreamFrame, RESOLUTION, SCHEMA};

/// Deterministic synthetic capture: per-channel baselines with a slow linear drift and a
/// small periodic perturbation; every `target_every`-th frame is target-present.
fn drifting_capture(n: u64, target_every: u64) -> Capture {
    Capture {
        schema_version: 1,
        capture_id: "fixture-synthetic-not-hardware".into(),
        device_class: "mock".into(),
        geometry_id: "mock-four-electrodes".into(),
        space: CaptureSpace::Synthetic2d,
        unit: CaptureUnit::PfPerMeter,
        channel_pairs: triad::forward::CHANNELS.map(|(a, b)| [a, b]),
        frames: (0..n)
            .map(|i| {
                let target = target_every > 0 && i % target_every == target_every - 1;
                CaptureFrame {
                    timestamp_ms: 1000 + i * 100,
                    role: if target {
                        FrameRole::TargetPresent
                    } else {
                        FrameRole::EmptyReference
                    },
                    channels: std::array::from_fn(|j| {
                        let base = 0.4 - 0.05 * j as f64;
                        let drift = 1e-5 * (j as f64 + 1.0) * i as f64;
                        let wiggle = 1e-4 * (((i * 7 + j as u64 * 3) % 11) as f64 - 5.0);
                        base + drift + wiggle + if target { 5e-3 } else { 0.0 }
                    }),
                }
            })
            .collect(),
    }
}

fn example() -> Capture {
    serde_json::from_str(include_str!("../examples/synthetic-capture.json")).unwrap()
}

#[test]
fn final_rolling_statistics_equal_the_batch_audit_exactly() {
    // With a window holding every reference, the causal statistics after the last frame
    // must reproduce Capture::audit bit for bit (same formula, same summation order).
    for capture in [example(), drifting_capture(240, 7)] {
        let audit = capture.audit().unwrap();
        let config = StreamConfig {
            window: 100_000,
            initial_reference_frames: 3,
        };
        let report = stream(&capture, config).unwrap();
        let last = report.frames.last().unwrap();
        assert_eq!(
            last.reference_frames_seen as usize,
            audit.empty_reference_frames
        );
        assert_eq!(last.reference_mean.unwrap(), audit.empty_mean);
        assert_eq!(last.reference_stddev.unwrap(), audit.empty_sample_stddev);
    }
}

#[test]
fn features_never_depend_on_future_frames() {
    let capture = drifting_capture(120, 5);
    let config = StreamConfig {
        window: 16,
        initial_reference_frames: 4,
    };
    let full = stream(&capture, config).unwrap();
    for cut in [10usize, 37, 64, 99] {
        // Truncate after frame `cut`, and separately replace every later frame with
        // very different values: frames 0..=cut must not change in either case.
        let mut truncated = capture.clone();
        truncated.frames.truncate(cut + 1);
        let mut altered = capture.clone();
        for f in altered.frames.iter_mut().skip(cut + 1) {
            f.channels = f.channels.map(|c| c * 3.0 + 1.0);
            f.role = FrameRole::TargetPresent;
        }
        altered.frames.last_mut().unwrap().role = FrameRole::EmptyReference;
        let a = stream(&truncated, config).unwrap();
        let b = stream(&altered, config).unwrap();
        assert_eq!(&a.frames[..], &full.frames[..=cut], "truncated at {cut}");
        assert_eq!(
            &b.frames[..=cut],
            &full.frames[..=cut],
            "altered after {cut}"
        );
    }
    // Sanity: the batch audit, by contrast, does change when later frames change.
    let mut altered = capture.clone();
    for f in altered.frames.iter_mut().skip(100) {
        f.channels = f.channels.map(|c| c + 1.0);
    }
    assert_ne!(
        capture.audit().unwrap().late_minus_early_empty_mean,
        altered.audit().unwrap().late_minus_early_empty_mean
    );
}

#[test]
fn repeated_identical_reference_frames_each_count() {
    let mut capture = drifting_capture(6, 0);
    let first = capture.frames[0].channels;
    for f in capture.frames.iter_mut() {
        f.channels = first;
    }
    let report = stream(&capture, StreamConfig::default()).unwrap();
    let last = report.frames.last().unwrap();
    assert_eq!(last.reference_frames_seen, 6);
    assert_eq!(last.reference_frames_in_window, 6);
    // Reported statistics stay raw (identical to Capture::audit), so the sd of identical
    // readings is 0 up to rounding of the mean; the normalized score applies the floor.
    let mean = last.reference_mean.unwrap();
    for (sd, m) in last.reference_stddev.unwrap().iter().zip(mean) {
        assert!(*sd <= RESOLUTION * m.abs(), "{sd} vs mean {m}");
    }
    // Identical readings: zero drift over zero noise is 0, not undefined.
    assert_eq!(last.drift_score, Some(0.0));
}

#[test]
fn rolling_window_wraps_to_the_most_recent_references() {
    let capture = drifting_capture(40, 0);
    let w = 5;
    let report = stream(
        &capture,
        StreamConfig {
            window: w,
            initial_reference_frames: 3,
        },
    )
    .unwrap();
    for (k, frame) in report.frames.iter().enumerate() {
        assert_eq!(frame.reference_frames_in_window, (k + 1).min(w));
        let recent = &capture.frames[(k + 1).saturating_sub(w)..=k];
        for j in 0..10 {
            let mean = recent.iter().map(|f| f.channels[j]).sum::<f64>() / recent.len() as f64;
            assert_eq!(frame.reference_mean.unwrap()[j], mean, "frame {k} ch {j}");
        }
    }
}

#[test]
fn drift_is_measured_from_the_initial_baseline() {
    let capture = drifting_capture(60, 0);
    let k = 4;
    let report = stream(
        &capture,
        StreamConfig {
            window: 8,
            initial_reference_frames: k,
        },
    )
    .unwrap();
    for frame in &report.frames[..k - 1] {
        assert_eq!(frame.drift_from_initial, None, "baseline not yet complete");
        assert_eq!(frame.drift_score, None);
    }
    let initial: [f64; 10] = std::array::from_fn(|j| {
        capture.frames[..k]
            .iter()
            .map(|f| f.channels[j])
            .sum::<f64>()
            / k as f64
    });
    let last = report.frames.last().unwrap();
    let means = last.reference_mean.unwrap();
    let drifts = last.drift_from_initial.unwrap();
    for ((m, init), d) in means.iter().zip(initial).zip(drifts) {
        let expected = m - init;
        assert!((d - expected).abs() <= 1e-15);
        assert!(expected > 0.0, "fixture drifts upward on every channel");
    }
    assert!(last.drift_score.unwrap() > 0.0);
}

#[test]
fn target_deviation_uses_reference_statistics_from_before_the_frame() {
    let capture = drifting_capture(30, 6);
    let report = stream(&capture, StreamConfig::default()).unwrap();
    for (k, frame) in report.frames.iter().enumerate() {
        let is_target = matches!(frame.role, FrameRole::TargetPresent);
        assert_eq!(frame.target_z.is_some(), is_target);
        assert_eq!(frame.max_abs_target_z.is_some(), is_target);
        if !is_target {
            continue;
        }
        let refs: Vec<_> = capture.frames[..k]
            .iter()
            .filter(|f| matches!(f.role, FrameRole::EmptyReference))
            .collect();
        let previous = &report.frames[k - 1];
        assert_eq!(
            frame.reference_mean, previous.reference_mean,
            "target does not update reference"
        );
        for j in 0..10 {
            let m = refs.iter().map(|f| f.channels[j]).sum::<f64>() / refs.len() as f64;
            let sd = (refs
                .iter()
                .map(|f| (f.channels[j] - m).powi(2))
                .sum::<f64>()
                / (refs.len() - 1) as f64)
                .sqrt();
            let z = (capture.frames[k].channels[j] - m) / sd;
            assert!((frame.target_z.unwrap()[j].unwrap() - z).abs() <= 1e-9 * z.abs().max(1.0));
        }
        assert!(
            frame.max_abs_target_z.unwrap() > 3.0,
            "5e-3 offset is many reference sds"
        );
    }
}

#[test]
fn insufficient_evidence_is_reported_as_null_not_zero() {
    let mut capture = drifting_capture(8, 0);
    capture.frames[0].role = FrameRole::TargetPresent; // target before any reference
    let report = stream(&capture, StreamConfig::default()).unwrap();
    let first: &StreamFrame = &report.frames[0];
    assert_eq!(first.reference_frames_seen, 0);
    assert_eq!(first.ms_since_reference, None);
    assert_eq!(first.reference_mean, None);
    assert_eq!(first.target_z, Some([None; 10]));
    assert_eq!(first.max_abs_target_z, None);
    let second = &report.frames[1];
    assert!(second.reference_mean.is_some());
    assert_eq!(
        second.reference_stddev, None,
        "one reference has no spread estimate"
    );
}

#[test]
fn staleness_counts_time_since_the_latest_reference() {
    let mut capture = drifting_capture(10, 0);
    for f in capture.frames.iter_mut().skip(4) {
        f.role = FrameRole::TargetPresent;
    }
    capture.frames.last_mut().unwrap().role = FrameRole::EmptyReference;
    let report = stream(&capture, StreamConfig::default()).unwrap();
    let since: Vec<_> = report.frames.iter().map(|f| f.ms_since_reference).collect();
    assert_eq!(
        since,
        [0.0, 0.0, 0.0, 0.0, 100.0, 200.0, 300.0, 400.0, 500.0, 0.0].map(Some)
    );
}

#[test]
fn zero_noise_channels_follow_the_resolution_rule() {
    // Channels that never vary give sd = 0. A target equal to the mean (within the
    // rounding floor) is 0 sds away; a target that differs is undefined, never infinite.
    let mut capture = drifting_capture(5, 0);
    for f in capture.frames.iter_mut() {
        f.channels[3] = 0.45; // three equal 0.45 readings do not average to exactly 0.45
        f.channels[4] = -0.1;
    }
    capture.frames[4].role = FrameRole::TargetPresent;
    capture.frames[4].channels[4] = -0.095;
    let report = stream(&capture, StreamConfig::default()).unwrap();
    let z = report.frames[4].target_z.unwrap();
    assert_eq!(z[3], Some(0.0));
    assert_eq!(z[4], None);
    assert!(z.iter().flatten().all(|v| v.is_finite()));
    // The floor is relative to the channel magnitude (RESOLUTION * 0.4 is about 1.4e-15):
    // a 1e-12 deviation on a zero-noise channel is real, not rounding, so it stays undefined.
    let mut capture = drifting_capture(5, 0);
    for f in capture.frames.iter_mut() {
        f.channels[3] = 0.4;
    }
    capture.frames[4].role = FrameRole::TargetPresent;
    capture.frames[4].channels[3] = 0.4 + 1e-12;
    let report = stream(&capture, StreamConfig::default()).unwrap();
    assert_eq!(report.frames[4].target_z.unwrap()[3], None);
}

#[test]
fn invalid_inputs_are_refused_before_streaming() {
    let mut capture = drifting_capture(10, 0);
    capture.frames[5].timestamp_ms = capture.frames[4].timestamp_ms;
    assert!(stream(&capture, StreamConfig::default()).is_err());
    let mut capture = drifting_capture(10, 0);
    capture.frames[2].channels[0] = f64::NAN;
    assert!(stream(&capture, StreamConfig::default()).is_err());
    let capture = drifting_capture(10, 0);
    for bad in [
        StreamConfig {
            window: 1,
            initial_reference_frames: 3,
        },
        StreamConfig {
            window: 8,
            initial_reference_frames: 0,
        },
    ] {
        assert!(stream(&capture, bad).is_err(), "{bad:?}");
    }
    let mut capture = drifting_capture(10, 0);
    capture.frames[9].timestamp_ms = u64::MAX;
    assert!(
        stream(&capture, StreamConfig::default()).is_err(),
        "ns overflow"
    );
}

#[test]
fn report_is_deterministic_and_self_describing() {
    let capture = drifting_capture(80, 9);
    let a = serde_json::to_string(&stream(&capture, StreamConfig::default()).unwrap()).unwrap();
    let b = serde_json::to_string(&stream(&capture, StreamConfig::default()).unwrap()).unwrap();
    assert_eq!(a, b);
    let report = stream(&capture, StreamConfig::default()).unwrap();
    assert_eq!(report.schema_version, SCHEMA);
    assert_eq!(report.engine.name, "flexplan");
    let reference = &report.engine.plans[0];
    assert_eq!(reference.event, "empty_reference");
    assert!(
        reference.steps.iter().all(|s| s.ends_with("handle")),
        "{:?}",
        reference.steps
    );
    let target = &report.engine.plans[1];
    let handle = target
        .steps
        .iter()
        .position(|s| s == "target handle")
        .unwrap();
    let last_stat = target
        .steps
        .iter()
        .rposition(|s| s.starts_with("reference.") && s.ends_with("eval"))
        .unwrap();
    assert!(
        last_stat < handle,
        "statistics are evaluated before the target handler"
    );
    assert!(
        !target.steps.iter().any(|s| s == "reference handle"),
        "targets never update the reference"
    );
    assert!(report
        .warning
        .contains("unvalidated candidate refusal score"));
    // Round-trips through the declared schema (deny_unknown_fields).
    let parsed: triad::capture_stream::StreamReport = serde_json::from_str(&a).unwrap();
    assert_eq!(parsed, report);
}
