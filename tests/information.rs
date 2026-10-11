use triad::{
    forward::Model,
    inverse::Dictionary,
    observability::local_position_information,
};

fn dictionary(confound_material: bool) -> Dictionary {
    let axes = [
        vec![-0.024, 0.0, 0.024],
        vec![0.008, 0.018, 0.028],
        vec![2.0, 5.0, 8.0],
    ];
    let mut states = Vec::new();
    let mut signals = Vec::new();
    for &x in &axes[0] {
        for &y in &axes[1] {
            for &er in &axes[2] {
                let mut signal = [0.0_f64; 10];
                signal[0] = 50.0 * x + if confound_material { 0.02 * er } else { 0.0 };
                signal[1] = 50.0 * y;
                if !confound_material {
                    signal[2] = 0.02 * er;
                }
                states.push([x, y, er]);
                signals.push(signal);
            }
        }
    }
    Dictionary {
        model: Model::new(0.001),
        axes,
        states,
        signals,
        cache_key: "synthetic-test".into(),
    }
}

#[test]
fn independent_material_measurement_preserves_position_information() {
    let d = dictionary(false);
    let result = local_position_information(&d, [0.002, 0.016, 4.0], 24);
    assert!(result.material_eliminated);
    assert!(result.nuisance_adjusted_weakest_per_mm2 > 0.0);
    let relative = (result.unadjusted_weakest_per_mm2
        - result.nuisance_adjusted_weakest_per_mm2)
        .abs()
        / result.unadjusted_weakest_per_mm2;
    assert!(relative < 1e-8, "unexpected nuisance penalty: {relative}");
}

#[test]
fn dielectric_confounder_can_destroy_local_position_information() {
    let d = dictionary(true);
    let result = local_position_information(&d, [0.002, 0.016, 4.0], 24);
    assert!(result.unadjusted_weakest_per_mm2 > 1e-4);
    assert!(
        result.nuisance_adjusted_weakest_per_mm2
            < result.unadjusted_weakest_per_mm2 * 1e-6,
        "material nuisance should eliminate x information: {result:?}"
    );
    assert!(result.nuisance_adjusted_condition.is_none());
}

#[test]
fn evaluation_position_does_not_require_hidden_truth() {
    let d = dictionary(true);
    let first = local_position_information(&d, [0.002, 0.016, 4.0], 24);
    let second = local_position_information(&d, [0.002, 0.016, 4.0], 24);
    assert_eq!(
        first.nuisance_adjusted_weakest_per_mm2.to_bits(),
        second.nuisance_adjusted_weakest_per_mm2.to_bits()
    );
}
