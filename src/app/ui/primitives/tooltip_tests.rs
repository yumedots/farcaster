//! Lifecycle rules for the shared tooltip overlay.
//!
//! The overlay lives in the vendored `gpui-base` crate, whose own test module
//! cannot build (its tests need gpui's `test-support` feature, which a
//! dependency build never enables). The behaviour is pinned here instead,
//! through the same public API the app's `AppTooltip` uses.

use std::time::Duration;

use gpui::{
    AppContext as _, Bounds, Entity, IntoElement, Pixels, TestAppContext, VisualTestContext, point,
    px, size,
};
use gpui_base::{TooltipOverlay, TooltipRequest};

/// How long the pointer must rest on a trigger. Mirrors the overlay's delay;
/// the test would rather fail loudly than drift.
const SHOW_DELAY: Duration = Duration::from_millis(400);
/// How long a tooltip stays up once the pointer leaves its trigger.
const HIDE_DELAY: Duration = Duration::from_millis(600);

/// A stand-in for the app's tooltip content. The overlay owns no presentation,
/// so the tests only need a view that can be built and drawn.
struct Popup;

impl gpui::Render for Popup {
    fn render(
        &mut self,
        _: &mut gpui::Window,
        _: &mut gpui::Context<'_, Self>,
    ) -> impl IntoElement {
        gpui::Empty
    }
}

fn bounds(x: f32, y: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(20.0), px(20.0)))
}

fn request(x: f32, y: f32) -> TooltipRequest {
    TooltipRequest::new(bounds(x, y), |_, cx| cx.new(|_| Popup).into())
}

/// Draws a frame, moves the clock on, and drains what became ready. A drawn
/// frame is what polls the overlay's countdown in the first place.
fn pump(cx: &mut VisualTestContext, delay: Duration) {
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    cx.executor().advance_clock(delay);
    cx.run_until_parked();
}

fn show(cx: &mut VisualTestContext, overlay: &Entity<TooltipOverlay>, x: f32, y: f32) {
    let overlay = overlay.clone();
    cx.update(|window, cx| {
        overlay.update(cx, |overlay, cx| {
            overlay.request_show(request(x, y), window, cx);
        });
    });
}

fn hide(cx: &mut VisualTestContext, overlay: &Entity<TooltipOverlay>) {
    let overlay = overlay.clone();
    cx.update(|window, cx| {
        overlay.update(cx, |overlay, cx| overlay.request_hide(window, cx));
    });
}

fn visible(cx: &mut VisualTestContext, overlay: &Entity<TooltipOverlay>) -> bool {
    let overlay = overlay.clone();
    cx.update(|_, cx| overlay.read(cx).is_visible())
}

#[gpui::test]
fn a_tooltip_waits_before_it_appears(cx: &mut TestAppContext) {
    let (overlay, cx) = cx.add_window_view(|_, _| TooltipOverlay::new());

    show(cx, &overlay, 0.0, 0.0);
    pump(cx, Duration::ZERO);
    assert!(
        !visible(cx, &overlay),
        "a tooltip appears only after the pointer rests on its trigger"
    );

    pump(cx, SHOW_DELAY - Duration::from_millis(50));
    assert!(!visible(cx, &overlay), "the delay is still running");

    pump(cx, Duration::from_millis(100));
    assert!(visible(cx, &overlay), "the tooltip appears once it elapses");
}

#[gpui::test]
fn a_sibling_trigger_starts_its_own_countdown(cx: &mut TestAppContext) {
    let (overlay, cx) = cx.add_window_view(|_, _| TooltipOverlay::new());

    show(cx, &overlay, 0.0, 0.0);
    pump(cx, SHOW_DELAY);
    assert!(visible(cx, &overlay));

    // Sweeping onto the next row takes the previous tooltip down and waits,
    // instead of flashing the new row's tooltip the moment the pointer arrives.
    show(cx, &overlay, 0.0, 40.0);
    assert!(!visible(cx, &overlay));

    pump(cx, SHOW_DELAY);
    assert!(visible(cx, &overlay));
}

#[gpui::test]
fn the_same_trigger_keeps_what_is_already_up(cx: &mut TestAppContext) {
    let (overlay, cx) = cx.add_window_view(|_, _| TooltipOverlay::new());

    show(cx, &overlay, 0.0, 0.0);
    pump(cx, SHOW_DELAY);
    assert!(visible(cx, &overlay));

    // A re-render inside the same row must not take the tooltip down.
    show(cx, &overlay, 0.0, 0.0);
    assert!(visible(cx, &overlay));
}

#[gpui::test]
fn the_same_trigger_shows_again_after_it_hides(cx: &mut TestAppContext) {
    let (overlay, cx) = cx.add_window_view(|_, _| TooltipOverlay::new());

    show(cx, &overlay, 0.0, 0.0);
    pump(cx, SHOW_DELAY);
    assert!(visible(cx, &overlay));

    // The pointer leaves the row and the tooltip goes away.
    hide(cx, &overlay);
    pump(cx, HIDE_DELAY);
    assert!(!visible(cx, &overlay));

    // Coming back to the same row is a fresh visit, not a request to keep
    // something that is no longer there: it waits, then shows again.
    show(cx, &overlay, 0.0, 0.0);
    pump(cx, Duration::ZERO);
    assert!(!visible(cx, &overlay), "the countdown starts over");

    pump(cx, SHOW_DELAY);
    assert!(visible(cx, &overlay), "the same trigger shows again");
}

#[gpui::test]
fn a_tooltip_lingers_then_dismisses(cx: &mut TestAppContext) {
    let (overlay, cx) = cx.add_window_view(|_, _| TooltipOverlay::new());

    show(cx, &overlay, 0.0, 0.0);
    pump(cx, SHOW_DELAY);

    // Leaving the trigger keeps the tooltip up briefly, so a pointer that
    // crosses a boundary does not make it flicker.
    hide(cx, &overlay);
    assert!(visible(cx, &overlay));

    pump(cx, HIDE_DELAY);
    assert!(!visible(cx, &overlay));

    // Dismissal resets the countdown, so the next trigger waits again.
    cx.update(|_, cx| overlay.update(cx, TooltipOverlay::hide));
    show(cx, &overlay, 0.0, 0.0);
    pump(cx, SHOW_DELAY - Duration::from_millis(50));
    assert!(!visible(cx, &overlay));

    pump(cx, Duration::from_millis(100));
    assert!(visible(cx, &overlay));
}
