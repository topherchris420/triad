use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn quantile(values: &[f64], p: f64) -> f64 {
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    let t = (v.len() - 1) as f64 * p;
    let a = t.floor() as usize;
    let b = t.ceil() as usize;
    v[a] + (t - a as f64) * (v[b] - v[a])
}
#[derive(Serialize, Deserialize)]
pub struct Metrics {
    pub samples: usize,
    pub sequences: usize,
    pub coverage: f64,
    pub mean_mm: f64,
    pub median_mm: f64,
    pub p90_mm: f64,
    pub over_5mm_rate: f64,
    pub mean_ci95_mm: [f64; 2],
    pub optimizer_nonconvergence_rate: f64,
}

pub fn summarize(errors: &[f64], ids: &[u64], nonconverged: usize) -> Metrics {
    let unique: std::collections::BTreeSet<_> = ids.iter().copied().collect();
    let groups: Vec<Vec<f64>> = unique
        .iter()
        .map(|id| {
            errors
                .iter()
                .zip(ids)
                .filter_map(|(e, i)| if i == id { Some(*e) } else { None })
                .collect()
        })
        .collect();
    let mut rng = ChaCha8Rng::seed_from_u64(812);
    let mut means = Vec::new();
    for _ in 0..2000 {
        let mut sum = 0.0;
        let mut n = 0;
        for _ in &groups {
            let group = &groups[rng.gen_range(0..groups.len())];
            sum += group.iter().sum::<f64>();
            n += group.len();
        }
        means.push(sum / n as f64);
    }
    Metrics {
        samples: errors.len(),
        sequences: unique.len(),
        coverage: 1.0,
        mean_mm: errors.iter().sum::<f64>() / errors.len() as f64,
        median_mm: quantile(errors, 0.5),
        p90_mm: quantile(errors, 0.9),
        over_5mm_rate: errors.iter().filter(|e| **e > 5.0).count() as f64 / errors.len() as f64,
        mean_ci95_mm: [quantile(&means, 0.025), quantile(&means, 0.975)],
        optimizer_nonconvergence_rate: nonconverged as f64 / errors.len() as f64,
    }
}
