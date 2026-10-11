use triad::{adaptive::refine, forward::Model, inverse::Dictionary};

#[test]
fn optional_edge_refinement_preserves_physical_solver_checks() {
    let model = Model::new(0.002);
    let state = Some([-0.0213, 0.0071, 4.0]);
    let a = model.solve(state).unwrap();
    let b = model
        .solve_refined(state, triad::forward::ElectrodeLayout::default(), true)
        .unwrap();
    assert!(b.nodes > a.nodes);
    assert!(
        b.free_residual < 1e-9 && b.conservation < 1e-9,
        "residual {}, conservation {}",
        b.free_residual,
        b.conservation
    );
    assert!(b.signal.iter().all(|x| x.is_finite()));
    assert!(Model::new(f64::NAN).solve(None).is_err());
}

#[test]
fn numerical_protocol_is_exact_and_new_probes_are_disjoint() {
    let mut p: serde_json::Value =
        serde_json::from_str(include_str!("../experiments/metrology-numerical-v5.json")).unwrap();
    triad::metrology_numerical::validate_protocol(&p).unwrap();
    p["background_drift"] = serde_json::json!(1.5);
    assert!(triad::metrology_numerical::validate_protocol(&p).is_err());
    for s in triad::metrology_numerical::PROBES {
        assert!(!triad::numerical_components::DEVELOPMENT_PROBES.contains(&s));
    }
}
#[test]
fn bounded_measurement_only_search_and_cache() {
    let axes = [vec![0.0, 1.0, 2.0], vec![0.0, 1.0], vec![2.0, 3.0]];
    let sample = |s: [f64; 3]| {
        let mut r = [0.0; 10];
        r[..3].copy_from_slice(&s);
        Ok(r)
    };
    let mut states = Vec::new();
    let mut signals = Vec::new();
    for &x in &axes[0] {
        for &y in &axes[1] {
            for &e in &axes[2] {
                let s = [x, y, e];
                states.push(s);
                signals.push(sample(s).unwrap());
            }
        }
    }
    let d = Dictionary {
        model: Model::new(0.001),
        axes,
        states,
        signals,
        cache_key: "synthetic".into(),
    };
    let observation = sample([0.5, 0.5, 2.5]).unwrap();
    let r = refine(&d, &observation, 2, 60, sample).unwrap();
    assert_eq!(r.candidates[0].state, [0.5, 0.5, 2.5]);
    assert_eq!(r.forward_solves, 45);
    assert_eq!(r.cache_hits, 9);
    assert!(r.abstain);
    let r = refine(&d, &observation, 2, 5, sample).unwrap();
    assert_eq!(r.forward_solves, 5);
    assert_eq!(r.numerical_status, "budget_exhausted");
    assert!(refine(&d, &[f64::NAN; 10], 2, 5, sample).is_err());
}
