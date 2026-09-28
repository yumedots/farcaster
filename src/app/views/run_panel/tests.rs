use super::resize::clamped_run_panel_width;
use crate::app::ui::theme::theme;

#[test]
fn run_panel_resize_stays_within_design_bounds() {
    assert_eq!(clamped_run_panel_width(100.0), theme().layout.run_panel_min);
    assert_eq!(clamped_run_panel_width(332.0), theme().size(332.0));
    assert_eq!(clamped_run_panel_width(500.0), theme().layout.run_panel_max);
}
