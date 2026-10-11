use triad::numerical_components::local_patch;

#[test]
fn local_patch_exact_for_trilinear_synthetic_signal() {
    let axes = [
        vec![-0.024, -0.006, 0.024],
        vec![0.008, 0.018, 0.028],
        vec![2.0, 5.0, 8.0],
    ];
    let state = [-0.0113, 0.0127, 3.4];
    let f = |s: [f64; 3]| {
        let [x, y, e] = s;
        let u = x + 2.0 * y + 0.1 * e + 3.0 * x * y + 0.5 * x * e + y * e + 2.0 * x * y * e;
        Ok([u; 10])
    };
    let got = local_patch(&axes, state, 4, f).unwrap();
    let want = f(state).unwrap();
    for i in 0..10 {
        assert!((got[i] - want[i]).abs() < 1e-12);
    }
}

#[test]
fn local_patch_rejects_extrapolation_bad_axes_and_zero_subdivisions() {
    let axes = [vec![-0.024, 0.024], vec![0.008, 0.028], vec![2.0, 8.0]];
    let f = |_: [f64; 3]| Ok([0.0; 10]);
    assert!(local_patch(&axes, [-0.030, 0.014, 5.0], 4, f).is_err());
    assert!(local_patch(&axes, [0.0, 0.014, 5.0], 0, f).is_err());
    let broken = [vec![-0.024, 0.024], vec![0.018, 0.018], vec![2.0, 8.0]];
    assert!(local_patch(&broken, [0.0, 0.018, 5.0], 4, f).is_err());
}
