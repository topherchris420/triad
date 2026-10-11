use triad::capture::{
    Capture, CaptureFrame, CaptureSpace, CaptureUnit, FrameRole,
};

fn capture() -> Capture {
    Capture {
        schema_version: 1,
        capture_id: "fixture-synthetic-not-hardware".into(),
        device_class: "mock".into(),
        geometry_id: "mock-four-electrodes".into(),
        space: CaptureSpace::Synthetic2d,
        unit: CaptureUnit::PfPerMeter,
        channel_pairs: triad::forward::CHANNELS.map(|(a, b)| [a, b]),
        frames: (0..5)
            .map(|i| CaptureFrame {
                timestamp_ms: 1000 + i * 100,
                role: if i == 4 {
                    FrameRole::TargetPresent
                } else {
                    FrameRole::EmptyReference
                },
                channels: std::array::from_fn(|j| (j as f64) + i as f64),
            })
            .collect(),
    }
}
#[test]
fn capture_audit_produces_honest_empty_reference_variability() {
    let result = capture().audit().unwrap();
    assert_eq!(result.empty_reference_frames, 4);
    assert_eq!(result.target_present_frames, 1);
    assert!((result.empty_mean[0] - 1.5).abs() < 1e-12);
    assert!((result.empty_sample_covariance[0][0] - 5.0 / 3.0).abs() < 1e-12);
    assert!((result.late_minus_early_empty_mean[0] - 2.0).abs() < 1e-12);
    assert_eq!(result.last_timestamp_ms, 1400);
}
#[test]
fn hardware_p_f_is_a_separate_ingestion_contract() {
    let mut capture = capture();
    capture.space = CaptureSpace::Physical3d;
    assert!(capture.validate().is_err()); // mismatch pF/m
    capture.unit = CaptureUnit::Pf;
    assert!(capture.validate().is_ok()); // still no inverse bridge
}
#[test]
fn invalid_channel_order_timestamps_and_nonfinite_values_reject() {
    let mut recording = capture();
    recording.channel_pairs.swap(1, 2);
    assert!(recording.audit().is_err());
    let mut recording = capture();
    recording.frames[2].timestamp_ms = recording.frames[1].timestamp_ms;
    assert!(recording.audit().is_err());
    let mut recording = capture();
    recording.frames[0].channels[8] = f64::NAN;
    assert!(recording.audit().is_err());
    let mut recording = capture();
    for f in &mut recording.frames[1..3] {
        f.role = FrameRole::TargetPresent;
    }
    assert!(recording.audit().is_err());
}
#[test]
fn serde_capture_roundtrip_is_schema_strict() {
    let recording = capture();
    let encoded = serde_json::to_string(&recording).unwrap();
    let decoded: Capture = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.audit().unwrap().empty_reference_frames, 4);
    let extra = encoded.replace("\"schema_version\":1", "\"schema_version\":1,\"unknown\":true");
    assert!(serde_json::from_str::<Capture>(&extra).is_err());
}
