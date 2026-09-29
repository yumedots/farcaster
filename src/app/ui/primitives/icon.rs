use gpui::{
    Animation, AnimationExt as _, AnyElement, App, Bounds, CursorStyle, Div, ElementId,
    GlobalElementId, InspectorElementId, InteractiveElement as _, IntoElement, LayoutId,
    MouseButton, ParentElement as _, PathBuilder, Pixels, Rgba, Role, SharedString, Stateful,
    StatefulInteractiveElement as _, Style, Styled as _, Window, div, point, px,
};
use gpui_component::{Icon, IconNamed, Sizable as _};
use std::time::Duration;

use super::AppTooltip as _;
use crate::app::ui::theme::theme;

#[derive(Clone, Copy)]
pub(crate) enum AppIconSize {
    Inline,
    Control,
    Prominent,
}

impl AppIconSize {
    fn pixels(self) -> Pixels {
        match self {
            Self::Inline => theme().icons.inline,
            Self::Control => theme().icons.control,
            Self::Prominent => theme().icons.prominent,
        }
    }
}

pub(crate) fn app_icon(icon: impl IconNamed, size: AppIconSize) -> Icon {
    Icon::new(icon).with_size(size.pixels())
}

const WHEEL_SPOKES: usize = 8;

fn spoke_opacity(phase: f32, spoke: usize) -> f32 {
    let behind = (phase * WHEEL_SPOKES as f32 - spoke as f32).rem_euclid(WHEEL_SPOKES as f32);
    1.0 - behind / WHEEL_SPOKES as f32
}

struct ActivityWheel {
    size: Pixels,
    color: Rgba,
    phase: f32,
}

impl IntoElement for ActivityWheel {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl gpui::Element for ActivityWheel {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = self.size.into();
        style.size.height = self.size.into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        _cx: &mut App,
    ) {
        let center = bounds.center();
        let radius = bounds.size.width.min(bounds.size.height) / 2.0;
        let outer = radius * 0.85;
        let inner = radius * 0.35;
        let width = (radius * 0.24).max(px(1.0));
        for spoke in 0..WHEEL_SPOKES {
            let angle = -std::f32::consts::FRAC_PI_2
                + spoke as f32 * std::f32::consts::TAU / WHEEL_SPOKES as f32;
            let (sin, cos) = angle.sin_cos();
            let mut builder = PathBuilder::stroke(width);
            builder.move_to(point(center.x + inner * cos, center.y + inner * sin));
            builder.line_to(point(center.x + outer * cos, center.y + outer * sin));
            let Ok(path) = builder.build() else {
                continue;
            };
            window.paint_path(path, self.color.opacity(spoke_opacity(self.phase, spoke)));
        }
    }
}

pub(crate) fn native_spinner(
    id: impl Into<ElementId>,
    size: AppIconSize,
    color: Rgba,
) -> AnyElement {
    let pixels = size.pixels();
    div()
        .size(pixels)
        .flex_none()
        .child(
            ActivityWheel {
                size: pixels,
                color,
                phase: 0.0,
            }
            .with_animation(
                id,
                Animation::new(Duration::from_millis(800)).repeat(),
                |mut wheel, phase| {
                    wheel.phase = phase;
                    wheel
                },
            ),
        )
        .into_any_element()
}

pub(crate) fn icon_control(
    id: impl Into<ElementId>,
    accessible_label: impl Into<SharedString>,
) -> Stateful<Div> {
    let accessible_label = accessible_label.into();
    let tooltip_label = accessible_label.clone();
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(accessible_label)
        .tab_index(0)
        .size(theme().controls.icon_button)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme().radius)
        .focus_visible(|control| {
            control
                .border(theme().border)
                .border_color(theme().colors.accent)
        })
        .cursor(CursorStyle::PointingHand)
        .on_mouse_down(MouseButton::Left, super::preserve_pointer_focus)
        .app_tooltip(tooltip_label)
}

#[cfg(test)]
#[path = "icon_tests.rs"]
mod tests;
