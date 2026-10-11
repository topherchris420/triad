use triad::{forward::Model, inverse::Dictionary, selective::score_frame};

#[test]
fn correct_position_can_have_low_posterior_exceedance() {
    let axes = [vec![-0.02, 0.02], vec![0.008, 0.028], vec![2.0, 8.0]];
    let mut states = Vec::new();
    let mut signals = Vec::new();
    for &x in &axes[0] {
        for &y in &axes[1] {
            for &e in &axes[2] {
                states.push([x, y, e]);
                let mut signal = [0.0; 10];
                signal[0] = x;
                signal[1] = y;
                signal[2] = e;
                signals.push(signal);
            }
        }
    }
    let dictionary = Dictionary {
        model: Model::new(0.001),
        axes,
        states,
        signals,
        cache_key: "synthetic-selective-test".into(),
    };
    let state = [-0.02, 0.008, 2.0];
    let observed = dictionary.signal(state);
    let ambiguity = vec![0.0; dictionary.states.len()];
    let scored = score_frame(&dictionary, &observed, state, &ambiguity, 7, 0);
    assert!(scored.c2 >= 0.0 && scored.c2 < 0.05);
    assert!(scored.d.is_finite() && scored.d >= 0.0 && scored.d <= 1.0);
    assert_eq!(scored.sequence_id, 7);
}
