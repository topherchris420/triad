use triad::forward::{ElectrodeLayout, Model};

#[test]
fn layout_overload_preserves_the_historical_default_forward_model() {
    let model = Model::new(0.002);
    let state = [0.002,0.018,4.5];
    let baseline = model.solve(Some(state)).unwrap();
    let explicit = model.solve_with_layout(Some(state),ElectrodeLayout::default()).unwrap();
    assert_eq!(baseline.signal,explicit.signal);
    assert_eq!(baseline.capacitance,explicit.capacitance);
}
#[test]
fn electrode_layout_rejects_overlap_and_out_of_bounds() {
    let g = Model::new(0.002).geometry;
    assert!(ElectrodeLayout::default().validate(g).is_ok());
    assert!(ElectrodeLayout {centers:[-0.02,-0.019,0.01,0.03],half_width:0.004}
        .validate(g).is_err());
    assert!(ElectrodeLayout {centers:[-0.09,-0.01,0.01,0.03],half_width:0.004}
        .validate(g).is_err());
}
