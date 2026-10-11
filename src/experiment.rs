//! Frozen Milestone 2 experiment. Forward gate must pass before this entry point.
use crate::{
    evaluation::{hash, summarize},
    forward::{Geometry, Model},
    inverse::{Dictionary, Estimate, Estimator},
    measurements::{simulate, Channels},
    observability::{ambiguity, position_sensitivity},
    plots,
};
use anyhow::{ensure, Result};
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path, time::Instant};

#[derive(Serialize, Deserialize)]
pub struct Pool {
    pub kind: String,
    pub model: Model,
    pub states: Vec<[f64; 3]>,
    pub signals: Vec<Channels>,
    pub key: String,
}
fn pool(root: &Path, kind: &str, dictionary: &Dictionary) -> Result<Pool> {
    let mut rng = ChaCha8Rng::seed_from_u64(66017);
    let mut states = Vec::new();
    let mut model = Model::new(0.0005);
    if kind == "forward_mismatch" {
        model.geometry.radius *= 1.10;
    }
    for x in &dictionary.axes[0] {
        for y in &dictionary.axes[1] {
            let x = (*x + rng.gen_range(-0.0014..0.0014)).clamp(-0.024, 0.024);
            let mut y = (*y + rng.gen_range(-0.0012..0.0012)).clamp(0.008, 0.028);
            let e = match kind {
                "wide_dielectric" => rng.gen_range(1.3..12.0),
                "poor_observability" => rng.gen_range(2.0..3.2),
                _ => rng.gen_range(2.0..8.0),
            };
            if kind == "poor_observability" {
                y = rng.gen_range(0.025..0.028);
            }
            states.push([x, y, e]);
        }
    }
    if kind == "global_ambiguity" {
        let mut pairs = ambiguity(dictionary);
        pairs.sort_by(|a, b| a.distance_noise_units.total_cmp(&b.distance_noise_units));
        states.clear();
        for pair in pairs {
            for state in [pair.state, pair.alternative] {
                if !states.contains(&state) {
                    states.push(state);
                }
            }
            if states.len() >= 24 {
                break;
            }
        }
    }
    let key = hash(&serde_json::to_vec(&(
        "pool-fenris-conforming-v1",
        kind,
        model,
        &states,
    ))?);
    let file = root.join(format!("results/cache/pool-{}.json", &key[..20]));
    if file.exists() {
        let p: Pool = serde_json::from_slice(&std::fs::read(file)?)?;
        ensure!(p.key == key, "pool cache mismatch");
        return Ok(p);
    }
    let mut signals = Vec::new();
    for (i, s) in states.iter().enumerate() {
        signals.push(model.solve(Some(*s))?.signal);
        if i % 12 == 0 {
            println!("Refined pool {kind}: {}/{}", i + 1, states.len());
        }
    }
    let p = Pool {
        kind: kind.into(),
        model,
        states,
        signals,
        key,
    };
    std::fs::write(file, serde_json::to_vec(&p)?)?;
    Ok(p)
}

#[derive(Serialize, Deserialize)]
pub struct Prediction {
    pub sequence_id: u64,
    pub frame: usize,
    pub estimate: Estimate,
    pub runtime_us: u128,
}
#[derive(Serialize, Deserialize)]
pub struct EvaluationRow {
    pub scenario: String,
    pub sequence_id: u64,
    pub frame: usize,
    pub truth: [f64; 3],
    pub estimate: [f64; 3],
    pub error_mm: f64,
    pub residual: f64,
    pub converged: bool,
}

pub fn run(root: &Path) -> Result<()> {
    let start = Instant::now();
    let cfg_bytes = std::fs::read(root.join("experiments/config.json"))?;
    let cfg: Value = serde_json::from_slice(&cfg_bytes)?;
    let validation: Value =
        serde_json::from_slice(&std::fs::read(root.join("results/validation.json"))?)?;
    ensure!(
        validation["passed"] == true && validation["config_sha256"] == hash(&cfg_bytes),
        "Pass matching forward validation first"
    );
    std::fs::create_dir_all(root.join("results/cache"))?;
    let axes = std::array::from_fn(|i| {
        let name = ["x_grid_m", "y_grid_m", "permittivity_grid"][i];
        cfg[name]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_f64().unwrap())
            .collect()
    });
    let dictionary = Dictionary::generate(
        Model {
            spacing: cfg["dictionary_spacing_m"].as_f64().unwrap(),
            geometry: Geometry::default(),
        },
        axes,
        root.join("results/cache").to_str().unwrap(),
    )?;
    let aliases = [
        ("clean", "clean"),
        ("mild_drift", "clean"),
        ("strong_drift", "clean"),
        ("wide_dielectric", "wide_dielectric"),
        ("combined_mismatch", "wide_dielectric"),
        ("poor_observability", "poor_observability"),
        ("global_ambiguity", "global_ambiguity"),
        ("correlated_disturbance", "clean"),
        ("channel_disturbance", "clean"),
        ("forward_mismatch", "forward_mismatch"),
    ];
    let mut pools = BTreeMap::new();
    for (_, kind) in aliases {
        if !pools.contains_key(kind) {
            pools.insert(kind, pool(root, kind, &dictionary)?);
        }
    }
    let calibration_frames = cfg["calibration_frames"].as_u64().unwrap() as usize;
    let frames = cfg["frames_per_sequence"].as_u64().unwrap() as usize;
    let mut sequences = Vec::new();
    for (scenario_index, (scenario, kind)) in aliases.iter().enumerate() {
        let p = &pools[kind];
        for (seed_index, seed) in cfg["seeds"].as_array().unwrap().iter().enumerate() {
            for (split_index, split) in ["training", "calibration", "test"].iter().enumerate() {
                for sequence_index in 0..cfg["sequences_per_seed"][split].as_u64().unwrap() {
                    let id = seed.as_u64().unwrap()
                        + 1000000 * (scenario_index as u64 + 1)
                        + 100000 * (split_index as u64 + 1)
                        + 10000 * sequence_index
                        + 1000 * seed_index as u64;
                    let mut rng = ChaCha8Rng::seed_from_u64(id ^ 0x91919191);
                    let mut indices: Vec<_> = (0..p.states.len()).collect();
                    indices.shuffle(&mut rng);
                    let states = indices
                        .iter()
                        .cycle()
                        .take(frames)
                        .map(|i| p.states[*i])
                        .collect();
                    let signals: Vec<_> = indices
                        .iter()
                        .cycle()
                        .take(frames)
                        .map(|i| p.signals[*i])
                        .collect();
                    sequences.push(simulate(
                        &signals,
                        states,
                        id,
                        scenario,
                        split,
                        calibration_frames,
                    ));
                }
            }
        }
    }
    ensure!(
        sequences
            .iter()
            .map(|s| s.sequence_id)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == sequences.len(),
        "sequence IDs collide"
    );
    // Freeze full observations, calibration and hidden truth before any predictions.
    let data_bytes = serde_json::to_vec(&sequences)?;
    std::fs::write(root.join("results/dataset.json"), &data_bytes)?;
    let estimator = Estimator {
        dictionary: &dictionary,
        calibration_frames,
    };
    let mut predictions = Vec::new();
    let mut timing = Vec::new();
    for s in sequences.iter().filter(|s| s.split == "test") {
        for (frame, z) in s.estimator_inputs().iter().enumerate() {
            let tick = Instant::now();
            let estimate = estimator.estimate(z);
            let runtime_us = tick.elapsed().as_micros();
            timing.push(runtime_us as f64 / 1000.0);
            predictions.push(Prediction {
                sequence_id: s.sequence_id,
                frame,
                estimate,
                runtime_us,
            });
        }
    }
    std::fs::write(
        root.join("results/predictions.json"),
        serde_json::to_vec(&predictions)?,
    )?;
    // Evaluation begins after the predictions artifact is complete.
    let lookup: BTreeMap<_, _> = sequences.iter().map(|s| (s.sequence_id, s)).collect();
    let rows: Vec<_> = predictions
        .iter()
        .map(|p| {
            let s = lookup[&p.sequence_id];
            let truth = s.truth_states[p.frame];
            let estimate = p.estimate.state;
            EvaluationRow {
                scenario: s.scenario.clone(),
                sequence_id: p.sequence_id,
                frame: p.frame,
                truth,
                estimate,
                error_mm: 1000.0
                    * ((truth[0] - estimate[0]).powi(2) + (truth[1] - estimate[1]).powi(2)).sqrt(),
                residual: p.estimate.normalized_residual,
                converged: p.estimate.converged,
            }
        })
        .collect();
    let mut metrics = BTreeMap::new();
    for (scenario, _) in aliases {
        let r: Vec<_> = rows.iter().filter(|r| r.scenario == scenario).collect();
        let errors: Vec<_> = r.iter().map(|r| r.error_mm).collect();
        let ids: Vec<_> = r.iter().map(|r| r.sequence_id).collect();
        metrics.insert(
            scenario,
            summarize(&errors, &ids, r.iter().filter(|r| !r.converged).count()),
        );
    }
    let ambiguity = ambiguity(&dictionary);
    std::fs::write(
        root.join("results/ambiguity.json"),
        serde_json::to_vec_pretty(&ambiguity)?,
    )?;
    std::fs::write(
        root.join("results/evaluation.json"),
        serde_json::to_vec(&rows)?,
    )?;
    // Noiseless inverse self-check at off-grid points, using its own interpolation.
    let checks:Vec<_>=[[0.003,0.018,4.2],[-0.015,0.010,6.1],[0.019,0.025,2.7]].into_iter().map(|s| {
        let prediction=estimator.estimate(&dictionary.signal(s));
        json!({"state":s,"estimate":prediction.state,"error_mm":1000.0*((s[0]-prediction.state[0]).powi(2)+(s[1]-prediction.state[1]).powi(2)).sqrt(),
            "residual":prediction.normalized_residual,"converged":prediction.converged})
    }).collect();
    let estimator_passed = checks
        .iter()
        .all(|c| c["error_mm"].as_f64().unwrap() < 0.1 && c["residual"].as_f64().unwrap() < 0.01);
    let summary = json!({"protocol":cfg["protocol_version"],"config_sha256":hash(&cfg_bytes),"dataset_sha256":hash(&data_bytes),
        "dictionary_key":dictionary.cache_key,"dictionary_states":dictionary.states.len(),"sequences":sequences.len(),
        "total_observations":sequences.iter().map(|s|s.observations.len()).sum::<usize>(),"test_predictions":predictions.len(),
        "scenarios":metrics,"optimizer_self_checks":checks,"estimator_passed":estimator_passed,
        "minimum_ambiguity_noise_units":ambiguity.iter().map(|a|a.distance_noise_units).fold(f64::INFINITY,f64::min),
        "inference_median_ms":crate::evaluation::quantile(&timing,0.5),"inference_p90_ms":crate::evaluation::quantile(&timing,0.9),
        "experiment_seconds":start.elapsed().as_secs_f64(),"peak_memory_bytes":peak_memory(),
        "pool_sizes":pools.iter().map(|(k,p)|((*k).to_string(),p.states.len())).collect::<BTreeMap<_,_>>()});
    std::fs::write(
        root.join("results/summary.json"),
        serde_json::to_vec_pretty(&summary)?,
    )?;
    let mut csv=String::from("scenario,sequence_id,frame,true_x_m,true_y_m,true_er,estimate_x_m,estimate_y_m,estimate_er,error_mm,residual,converged\n");
    for r in &rows {
        csv.push_str(&format!(
            "{},{},{},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{}\n",
            r.scenario,
            r.sequence_id,
            r.frame,
            r.truth[0],
            r.truth[1],
            r.truth[2],
            r.estimate[0],
            r.estimate[1],
            r.estimate[2],
            r.error_mm,
            r.residual,
            r.converged
        ));
    }
    std::fs::write(root.join("results/localization.csv"), csv)?;
    plots::generate(root, &dictionary, &rows, &ambiguity, &summary)?;
    let mut sensitivity = Vec::new();
    for x in &dictionary.axes[0] {
        for y in &dictionary.axes[1] {
            sensitivity.push(json!({"x_m":x,"y_m":y,"er":5.0,"sensitivity_pf_per_m_per_mm":position_sensitivity(&dictionary,[*x,*y,5.0])}));
        }
    }
    std::fs::write(
        root.join("results/sensitivity.json"),
        serde_json::to_vec_pretty(&sensitivity)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    ensure!(estimator_passed, "Inverse noiseless self-check failed");
    Ok(())
}

#[cfg(windows)]
fn peak_memory() -> Option<usize> {
    #[repr(C)]
    struct Counters {
        cb: u32,
        faults: u32,
        peak_working: usize,
        working: usize,
        peak_paged: usize,
        paged: usize,
        peak_nonpaged: usize,
        nonpaged: usize,
        pagefile: usize,
        peak_pagefile: usize,
    }
    #[link(name = "psapi")]
    unsafe extern "system" {
        fn GetProcessMemoryInfo(
            process: *mut std::ffi::c_void,
            counters: *mut Counters,
            size: u32,
        ) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
    }
    let mut c = Counters {
        cb: std::mem::size_of::<Counters>() as u32,
        faults: 0,
        peak_working: 0,
        working: 0,
        peak_paged: 0,
        paged: 0,
        peak_nonpaged: 0,
        nonpaged: 0,
        pagefile: 0,
        peak_pagefile: 0,
    };
    let size = c.cb;
    if unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut c, size) } != 0 {
        Some(c.peak_working)
    } else {
        None
    }
}
#[cfg(not(windows))]
fn peak_memory() -> Option<usize> {
    None
}
