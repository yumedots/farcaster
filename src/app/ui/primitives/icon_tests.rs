use super::{WHEEL_SPOKES, spoke_opacity};

#[test]
fn wheel_uses_eight_spokes() {
    assert_eq!(WHEEL_SPOKES, 8);
}

#[test]
fn spoke_opacity_peaks_at_the_head_and_fades_ahead() {
    assert_eq!(spoke_opacity(0.0, 0), 1.0);
    assert!(spoke_opacity(0.0, 1) < 0.2);
    assert!(spoke_opacity(0.0, 1) < spoke_opacity(0.0, WHEEL_SPOKES - 1));
}

#[test]
fn spoke_opacity_stays_in_range_through_the_cycle() {
    for step in 0..40 {
        let phase = step as f32 / 40.0;
        for spoke in 0..WHEEL_SPOKES {
            let opacity = spoke_opacity(phase, spoke);
            assert!((0.0..=1.0).contains(&opacity));
        }
    }
}
