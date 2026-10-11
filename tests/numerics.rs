use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

#[test]
fn numerical_cache_roundtrips_float_bits_exactly() {
    let mut rng = ChaCha8Rng::seed_from_u64(773);
    let values: Vec<f64> = (0..20000)
        .map(|_| measurements::normal(&mut rng) * 0.003)
        .collect();
    let bytes = serde_json::to_vec(&values).unwrap();
    let loaded: Vec<f64> = serde_json::from_slice(&bytes).unwrap();
    assert!(values
        .iter()
        .zip(loaded)
        .all(|(a, b)| a.to_bits() == b.to_bits()));
}
use triad::{
    forward::Model,
    inverse::{Dictionary, Estimator},
    measurements::{self, Channels},
    validation,
};

#[test]
fn exact_affine_and_layered_analytic_solutions() {
    let b = validation::benchmarks().unwrap();
    assert!(b["affine_max_error"].as_f64().unwrap() < 1e-9);
    assert!(b["layered_relative_error"].as_f64().unwrap() < 1e-9);
}
#[test]
fn full_capacitance_has_physical_structure_and_charge_consistency() {
    let s = Model::new(0.001).solve(Some([0.003, 0.016, 5.0])).unwrap();
    assert!(s.conservation < 1e-9 && s.free_residual < 1e-9);
    for i in 0..5 {
        assert!(s.capacitance[i][i] > 0.0);
        for j in 0..5 {
            assert!((s.capacitance[i][j] - s.capacitance[j][i]).abs() < 1e-9);
            assert!((s.capacitance[i][j] - s.reaction_capacitance[i][j]).abs() < 1e-8);
            if i != j {
                assert!(s.capacitance[i][j] < 1e-9);
            }
        }
    }
    let c = fenris::nalgebra::SMatrix::<f64, 5, 5>::from_fn(|i, j| s.capacitance[i][j]);
    let eigen = c.symmetric_eigen();
    assert!(eigen.eigenvalues.min() > -1e-8);
    assert!(eigen.eigenvalues.min().abs() < 1e-8); // global-potential gauge mode
}
#[test]
fn matching_background_is_position_indistinguishable() {
    let model = Model::new(0.002);
    let a = model.solve(Some([-0.02, 0.01, 1.0])).unwrap();
    let b = model.solve(Some([0.02, 0.025, 1.0])).unwrap();
    assert_eq!(a.vector, b.vector);
    assert_eq!(a.signal, [0.0; 10]);
}
#[test]
fn correlated_noise_covariance_matches_declared_equation() {
    let mut rng = ChaCha8Rng::seed_from_u64(551);
    let n = 50000;
    let mut xx = 0.0;
    let mut xy = 0.0;
    for _ in 0..n {
        let x = measurements::noise(&mut rng);
        xx += x[0] * x[0];
        xy += x[0] * x[1];
    }
    let variance = measurements::SIGMA.powi(2);
    assert!((xx / n as f64 / variance - 1.0).abs() < 0.035);
    assert!((xy / n as f64 / variance - measurements::RHO).abs() < 0.025);
}
#[test]
fn whitening_matches_gls_precision() {
    let v: Channels = std::array::from_fn(|i| i as f64 * 0.001 - 0.003);
    let sigma = measurements::SIGMA;
    let rho = measurements::RHO;
    let covariance = fenris::nalgebra::SMatrix::<f64, 10, 10>::from_fn(|i, j| {
        sigma * sigma * ((if i == j { 1.0 - rho } else { 0.0 }) + rho) * (1.0 + 1.0 / 24.0)
    });
    let z = fenris::nalgebra::SVector::<f64, 10>::from_row_slice(&v);
    let exact = z.dot(&(covariance.try_inverse().unwrap() * z));
    let w = measurements::whiten(&v, 24);
    let q = w.iter().map(|x| x * x).sum::<f64>();
    assert!((q - exact).abs() < 1e-9);
}
#[test]
fn simulation_is_reproducible_and_inference_does_not_read_truth() {
    let signals = vec![[0.1; 10]; 12];
    let states = vec![[0.001, 0.018, 5.0]; 12];
    let a = measurements::simulate(&signals, states.clone(), 88, "strong_drift", "test", 24);
    let mut b = measurements::simulate(&signals, states, 88, "strong_drift", "test", 24);
    assert_eq!(a.observations, b.observations);
    let input = a.estimator_inputs();
    b.truth_states = vec![[999.0; 3]; 12];
    b.gain_truth = vec![[999.0; 10]; 12];
    assert_eq!(input, b.estimator_inputs());
    let c = measurements::simulate(&signals, vec![[0.0; 3]; 12], 89, "strong_drift", "test", 24);
    assert_ne!(a.observations, c.observations);
}
#[test]
fn estimator_recovers_continuous_position_and_unknown_material() {
    let axes = [
        vec![-0.024, 0.0, 0.024],
        vec![0.008, 0.018, 0.028],
        vec![2.0, 5.0, 8.0],
    ];
    let mut states = Vec::new();
    let mut signals = Vec::new();
    for x in &axes[0] {
        for y in &axes[1] {
            for e in &axes[2] {
                states.push([*x, *y, *e]);
                // Identifiable synthetic inverse benchmark, separate from FEM correctness.
                signals.push([
                    *x * 10.0,
                    *y * 10.0,
                    *e * 0.02,
                    *x * *e,
                    *y * *e,
                    *x * *y,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                ]);
            }
        }
    }
    let dictionary = Dictionary {
        model: Model::new(0.001),
        axes,
        states,
        signals,
        cache_key: "test".into(),
    };
    let estimator = Estimator {
        dictionary: &dictionary,
        calibration_frames: 24,
    };
    let truth = [-0.011, 0.022, 4.3];
    let e = estimator.estimate(&dictionary.signal(truth));
    assert!(e.normalized_residual < 1e-5);
    for i in 0..3 {
        assert!((e.state[i] - truth[i]).abs() < 1e-6, "state {:?}", e.state);
    }
}
#[test]
fn refined_target_signal_is_stable_at_off_grid_position() {
    let state = [0.0073, 0.0187, 4.2];
    let a = Model::new(0.001).solve(Some(state)).unwrap();
    let b = Model::new(0.0005).solve(Some(state)).unwrap();
    let relative = a
        .signal
        .iter()
        .zip(b.signal)
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
        / b.signal.iter().map(|x| x * x).sum::<f64>().sqrt();
    assert!(relative < 0.08, "relative signal mesh error {relative}");
}
