//! Read-only capture interchange and empirical empty-reference diagnostics.
//! Raw physical pF observations are NOT compatible with TRIAD's 2D pF/m
//! forward likelihood. This module never calls the inverse solver.
use crate::forward::CHANNELS;
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureSpace {
    Physical3d,
    Synthetic2d,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureUnit {
    Pf,
    PfPerMeter,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameRole {
    EmptyReference,
    TargetPresent,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureFrame {
    pub timestamp_ms: u64,
    pub role: FrameRole,
    /// Absolute capacitance in the declared unit; no silent subtraction,
    /// negative mutual-capacitance sign change, or per-length conversion.
    pub channels: [f64; 10],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capture {
    pub schema_version: u32,
    pub capture_id: String,
    pub device_class: String,
    pub geometry_id: String,
    pub space: CaptureSpace,
    pub unit: CaptureUnit,
    /// TRIAD's ten upper triangle channel pairs use zero-based electrode IDs.
    pub channel_pairs: [[usize; 2]; 10],
    pub frames: Vec<CaptureFrame>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaptureAudit {
    pub schema_version: u32,
    pub capture_id: String,
    pub device_class: String,
    pub geometry_id: String,
    pub space: CaptureSpace,
    pub unit: CaptureUnit,
    pub empty_reference_frames: usize,
    pub target_present_frames: usize,
    pub first_timestamp_ms: u64,
    pub last_timestamp_ms: u64,
    pub empty_mean: [f64; 10],
    pub empty_sample_stddev: [f64; 10],
    /// Sample covariance, including possible drift; not necessarily full-rank.
    pub empty_sample_covariance: [[f64; 10]; 10],
    /// Difference between the latter and former halves of empty references.
    pub late_minus_early_empty_mean: [f64; 10],
    pub warning: String,
}
impl Capture {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == 1,
            "unsupported capture schema version"
        );
        ensure!(
            !self.capture_id.trim().is_empty()
                && !self.device_class.trim().is_empty()
                && !self.geometry_id.trim().is_empty(),
            "capture_id, device_class and geometry_id are required"
        );
        let expected = CHANNELS.map(|(i, j)| [i, j]);
        ensure!(
            self.channel_pairs == expected,
            "channel_pairs must match TRIAD's explicit 0-based upper-triangle ordering"
        );
        ensure!(
            matches!(
                (self.space, self.unit),
                (CaptureSpace::Physical3d, CaptureUnit::Pf)
                    | (CaptureSpace::Synthetic2d, CaptureUnit::PfPerMeter)
            ),
            "physical3d uses pF; synthetic2d uses pF/m. No automatic conversion exists"
        );
        ensure!(!self.frames.is_empty(), "capture has no frames");
        let mut last = None;
        let mut reference_count = 0usize;
        for frame in &self.frames {
            if let Some(previous) = last {
                ensure!(
                    frame.timestamp_ms > previous,
                    "timestamps must be strictly increasing"
                );
            }
            last = Some(frame.timestamp_ms);
            ensure!(
                frame.channels.iter().all(|c| c.is_finite()),
                "non-finite measurement channel"
            );
            if matches!(frame.role, FrameRole::EmptyReference) {
                reference_count += 1;
            }
        }
        ensure!(
            reference_count >= 3,
            "at least three separate empty-reference frames are required"
        );
        Ok(())
    }
    /// Descriptive statistics only: does not certify iid noise, remove drift,
    /// estimate real positions or repair the failed 2D numerical readiness gate.
    pub fn audit(&self) -> Result<CaptureAudit> {
        self.validate()?;
        let empty: Vec<_> = self
            .frames
            .iter()
            .filter(|f| matches!(f.role, FrameRole::EmptyReference))
            .collect();
        let n = empty.len();
        let mean: [f64; 10] =
            std::array::from_fn(|j| empty.iter().map(|f| f.channels[j]).sum::<f64>() / n as f64);
        let covariance: [[f64; 10]; 10] = std::array::from_fn(|i| {
            std::array::from_fn(|j| {
                empty
                    .iter()
                    .map(|f| (f.channels[i] - mean[i]) * (f.channels[j] - mean[j]))
                    .sum::<f64>()
                    / (n - 1) as f64
            })
        });
        let stddev = std::array::from_fn(|i| covariance[i][i].max(0.0).sqrt());
        let split = n / 2;
        let late_minus_early_empty_mean = std::array::from_fn(|i| {
            empty[split..].iter().map(|f| f.channels[i]).sum::<f64>() / (n - split) as f64
                - empty[..split].iter().map(|f| f.channels[i]).sum::<f64>() / split as f64
        });
        Ok(CaptureAudit {
            schema_version: 1,
            capture_id: self.capture_id.clone(),
            device_class: self.device_class.clone(),
            geometry_id: self.geometry_id.clone(),
            space: self.space,
            unit: self.unit,
            empty_reference_frames: n,
            target_present_frames: self.frames.len() - n,
            first_timestamp_ms: self.frames.first().unwrap().timestamp_ms,
            last_timestamp_ms: self.frames.last().unwrap().timestamp_ms,
            empty_mean: mean,
            empty_sample_stddev: stddev,
            empty_sample_covariance: covariance,
            late_minus_early_empty_mean,
            warning: "Exploratory descriptive statistics only. Reference variation mixes electronic noise, drift and environmental effects; covariance may be singular. Raw pF readings MUST NOT enter the pF/m 2D inverse model. No real sensor validation or 3D simulation is established.".into(),
        })
    }
}
pub fn inspect_file(path: &Path) -> Result<CaptureAudit> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    ensure!(
        bytes.len() <= 50_000_000,
        "capture exceeds 50 MB inspection limit"
    );
    let capture: Capture = serde_json::from_slice(&bytes).context("parse TRIAD capture v1 JSON")?;
    capture.audit()
}
