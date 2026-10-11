//! Observations and permitted empty-target calibration; truth kept separately.
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rand_distr::StandardNormal;
use serde::{Deserialize, Serialize};
pub type Channels = [f64; 10];
pub const SIGMA: f64 = 0.003;
pub const RHO: f64 = 0.35;
pub fn normal(rng: &mut ChaCha8Rng) -> f64 {
    rng.sample(StandardNormal)
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Sequence {
    pub sequence_id: u64,
    pub split: String,
    pub scenario: String,
    pub observations: Vec<Channels>,
    pub calibration: Vec<Channels>,
    pub truth_states: Vec<[f64; 3]>,
    pub gain_truth: Vec<Channels>,
    pub offset_truth: Vec<Channels>,
    pub shared_truth: Vec<f64>,
}
pub fn noise(rng: &mut ChaCha8Rng) -> Channels {
    let common = normal(rng) * SIGMA * RHO.sqrt();
    std::array::from_fn(|_| common + normal(rng) * SIGMA * (1.0 - RHO).sqrt())
}
pub fn whiten(v: &Channels, calibration_frames: usize) -> Channels {
    let mean = v.iter().sum::<f64>() / 10.0;
    let factor = if calibration_frames == 0 {
        1.0
    } else {
        (1.0 + 1.0 / calibration_frames as f64).sqrt()
    };
    std::array::from_fn(|i| {
        (v[i] - mean) / (SIGMA * (1.0 - RHO).sqrt() * factor)
            + mean / (SIGMA * (1.0 - RHO + 10.0 * RHO).sqrt() * factor)
    })
}
pub fn simulate(
    signals: &[Channels],
    states: Vec<[f64; 3]>,
    seed: u64,
    scenario: &str,
    split: &str,
    calibration_frames: usize,
) -> Sequence {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let fixed: Channels = std::array::from_fn(|_| normal(&mut rng) * 0.01);
    let calibration = (0..calibration_frames)
        .map(|_| {
            let n = noise(&mut rng);
            std::array::from_fn(|i| fixed[i] + n[i])
        })
        .collect();
    let mut gain = [0.0; 10];
    let mut offset = [0.0; 10];
    let mut shared = 0.0;
    let strength = match scenario {
        "mild_drift" => 1.0,
        "strong_drift" | "combined_mismatch" => 4.0,
        _ => 0.0,
    };
    let channel = rng.gen_range(0..10);
    let mut sequence = Sequence {
        sequence_id: seed,
        split: split.into(),
        scenario: scenario.into(),
        observations: vec![],
        calibration,
        truth_states: states,
        gain_truth: vec![],
        offset_truth: vec![],
        shared_truth: vec![],
    };
    for (t, signal) in signals.iter().enumerate() {
        for i in 0..10 {
            gain[i] += strength * normal(&mut rng) * 0.004;
            offset[i] += strength * normal(&mut rng) * 0.0004;
        }
        shared += strength * normal(&mut rng) * 0.0007;
        if scenario == "correlated_disturbance" {
            shared += normal(&mut rng) * 0.002;
        }
        if scenario == "channel_disturbance" {
            offset[channel] = 0.025 * t as f64 / (signals.len() - 1) as f64;
        }
        let n = noise(&mut rng);
        sequence.observations.push(std::array::from_fn(|i| {
            (1.0 + gain[i]) * signal[i] + fixed[i] + offset[i] + shared + n[i]
        }));
        sequence.gain_truth.push(gain);
        sequence.offset_truth.push(offset);
        sequence.shared_truth.push(shared);
    }
    sequence
}
impl Sequence {
    pub fn estimator_inputs(&self) -> Vec<Channels> {
        let mean: Channels = std::array::from_fn(|i| {
            self.calibration.iter().map(|z| z[i]).sum::<f64>() / self.calibration.len() as f64
        });
        self.observations
            .iter()
            .map(|z| std::array::from_fn(|i| z[i] - mean[i]))
            .collect()
    }
}
