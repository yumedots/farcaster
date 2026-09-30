use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, App, AppContext as _, Bounds, CursorStyle, Div, Element,
    ElementId, EmptyView, EntityId, GlobalElementId, InspectorElementId, InteractiveElement as _,
    IntoElement, LayoutId, MouseButton, ParentElement as _, Pixels, Position,
    StatefulInteractiveElement as _, Style, Styled as _, WeakEntity, Window, div,
    prelude::FluentBuilder as _, px, relative,
};

use crate::app::{
    FarcasterApp,
    ui::theme::theme,
    workspace::{TerminalDropSide, TerminalLayout, TerminalPane, TerminalSplitDirection},
};

struct TerminalSplitResize {
    path: Vec<bool>,
}

struct TerminalPaneMove {
    pane: EntityId,
}

type PaneBounds = Rc<RefCell<HashMap<EntityId, Bounds<Pixels>>>>;

#[derive(Clone)]
struct HandleState {
    hovered: Option<EntityId>,
    dragging: Option<EntityId>,
    drag_active: bool,
    hide_borders: bool,
    handles: bool,
    appearing: Option<EntityId>,
    drop_side: Rc<RefCell<Option<(EntityId, TerminalDropSide)>>>,
    pane_bounds: PaneBounds,
}

impl FarcasterApp {
    pub(in crate::app::views) fn render_terminal_workspace(
        &self,
        entity: WeakEntity<Self>,
        drag_active: bool,
    ) -> AnyElement {
        let Some(layout) = self.active_terminal_layout() else {
            return div()
                .size_full()
                .min_h_0()
                .children(self.workspace.terminal.view.clone())
                .into_any_element();
        };
        let state = HandleState {
            hovered: self.workspace.terminal.hovered_handle,
            dragging: self.workspace.terminal.dragging_pane,
            drag_active,
            hide_borders: self.settings.hide_split_borders,
            handles: layout.leaf_count() > 1,
            appearing: self.workspace.terminal.appearing_pane,
            drop_side: self.workspace.terminal.drop_side.clone(),
            pane_bounds: self.workspace.terminal.pane_bounds.clone(),
        };
        div()
            .size_full()
            .min_h_0()
            .flex()
            .child(terminal_pane_element(
                layout,
                layout.root(),
                &entity,
                1.0,
                &[],
                &state,
            ))
            .into_any_element()
    }
}

fn terminal_pane_element(
    layout: &TerminalLayout,
    pane: &TerminalPane,
    entity: &WeakEntity<FarcasterApp>,
    grow: f32,
    path: &[bool],
    state: &HandleState,
) -> AnyElement {
    match pane {
        TerminalPane::Leaf(id) => {
            let pane_id = *id;
            let terminal = layout.terminal(pane_id).cloned();
            let click = entity.clone();
            let visible = state.hovered == Some(pane_id)
                || (state.drag_active && state.dragging == Some(pane_id));
            let indicator = state
                .drag_active
                .then(|| *state.drop_side.borrow())
                .flatten()
                .filter(|(target, _)| *target == pane_id)
                .map(|(_, side)| side);
            let leaf = div()
                .id(format!("terminal-pane-{pane_id}"))
                .relative()
                .flex()
                .flex_col()
                .flex_grow(grow)
                .flex_shrink_1()
                .flex_basis(px(0.0))
                .min_w_0()
                .min_h_0()
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    let _ =
                        click.update(cx, |this, cx| this.focus_terminal_pane(pane_id, window, cx));
                });
            let strip = state
                .handles
                .then(|| terminal_handle_strip(pane_id, visible, entity));
            let wrapper = div().flex_1().min_w_0().min_h_0().children(terminal);
            let fill = || div().flex_none().bg(theme().colors.drop_highlight);
            let preview = |fill: Div, side: TerminalDropSide| {
                fill.with_animation(
                    format!("terminal-drop-fill-{pane_id}-{}", side.label()),
                    Animation::new(Duration::from_millis(120))
                        .with_easing(|value| value * value * (3.0 - 2.0 * value)),
                    |fill, progress| fill.opacity(progress),
                )
            };
            let pane = match indicator {
                Some(TerminalDropSide::Up) => leaf
                    .child(preview(fill().h(relative(0.5)), TerminalDropSide::Up))
                    .children(strip)
                    .child(wrapper),
                Some(TerminalDropSide::Down) => leaf
                    .children(strip)
                    .child(wrapper)
                    .child(preview(fill().h(relative(0.5)), TerminalDropSide::Down)),
                Some(TerminalDropSide::Left) => leaf.children(strip).child(
                    div()
                        .flex()
                        .flex_1()
                        .min_w_0()
                        .min_h_0()
                        .child(preview(fill().w(relative(0.5)), TerminalDropSide::Left))
                        .child(wrapper),
                ),
                Some(TerminalDropSide::Right) => leaf.children(strip).child(
                    div()
                        .flex()
                        .flex_1()
                        .min_w_0()
                        .min_h_0()
                        .child(wrapper)
                        .child(preview(fill().w(relative(0.5)), TerminalDropSide::Right)),
                ),
                None => leaf.children(strip).child(wrapper),
            };
            let pane = pane.when(state.handles, |pane| {
                pane.child(PaneBoundsProbe {
                    pane_id,
                    bounds: Rc::clone(&state.pane_bounds),
                })
                .child(terminal_drop_zone(pane_id, entity, &state.pane_bounds))
            });
            if state.appearing == Some(pane_id) {
                pane.with_animation(
                    format!("terminal-appear-{pane_id}"),
                    Animation::new(Duration::from_millis(300))
                        .with_easing(|value| 1.0 - (1.0 - value).powi(3)),
                    move |pane, progress| pane.flex_grow(grow * progress),
                )
                .into_any_element()
            } else {
                pane.into_any_element()
            }
        }
        TerminalPane::Split {
            direction,
            ratio,
            first,
            second,
        } => {
            let row = *direction == TerminalSplitDirection::Right;
            let mut first_path = path.to_vec();
            first_path.push(false);
            let mut second_path = path.to_vec();
            second_path.push(true);
            let drag_path = path.to_vec();
            let drag_entity = entity.clone();
            let clear_entity = entity.clone();
            div()
                .flex_grow(grow)
                .flex_shrink_1()
                .flex_basis(px(0.0))
                .min_w_0()
                .min_h_0()
                .flex()
                .when(row, |pane| pane.flex_row())
                .when(!row, |pane| pane.flex_col())
                .on_drag_move::<TerminalSplitResize>(move |event, _window, cx| {
                    if event.drag(cx).path != drag_path {
                        return;
                    }
                    let bounds = event.bounds;
                    let position = event.event.position;
                    let ratio = if row {
                        (position.x - bounds.left()).as_f32() / bounds.size.width.as_f32()
                    } else {
                        (position.y - bounds.top()).as_f32() / bounds.size.height.as_f32()
                    };
                    let _ = drag_entity.update(cx, |this, cx| {
                        this.set_terminal_split_ratio(&drag_path, ratio, cx);
                    });
                })
                .child(terminal_pane_element(
                    layout,
                    first,
                    entity,
                    *ratio,
                    &first_path,
                    state,
                ))
                .child(terminal_divider(
                    row,
                    path,
                    state.hide_borders,
                    &clear_entity,
                ))
                .child(terminal_pane_element(
                    layout,
                    second,
                    entity,
                    1.0 - *ratio,
                    &second_path,
                    state,
                ))
                .into_any_element()
        }
    }
}

fn terminal_handle_strip(
    pane_id: EntityId,
    visible: bool,
    entity: &WeakEntity<FarcasterApp>,
) -> AnyElement {
    let hover_entity = entity.clone();
    let drag_entity = entity.clone();
    let strip = div()
        .id(format!("terminal-pane-strip-{pane_id}"))
        .flex_none()
        .w_full()
        .h(px(16.0))
        .flex()
        .items_center()
        .justify_center()
        .on_hover(move |hovered, _window, cx| {
            let _ = hover_entity.update(cx, |this, cx| {
                let hovered_handle = &mut this.workspace.terminal.hovered_handle;
                if *hovered {
                    if *hovered_handle != Some(pane_id) {
                        *hovered_handle = Some(pane_id);
                        cx.notify();
                    }
                } else if *hovered_handle == Some(pane_id) {
                    *hovered_handle = None;
                    cx.notify();
                }
            });
        });
    if !visible {
        return strip.into_any_element();
    }
    let mut dots = div().flex().items_center().gap(px(3.0));
    for _ in 0..3 {
        dots = dots.child(div().size(px(4.0)).rounded_full().bg(theme().colors.muted));
    }
    let pill = dots
        .id(format!("terminal-pane-handle-{pane_id}"))
        .px(px(6.0))
        .py(px(2.0))
        .rounded_full()
        .bg(theme().colors.surface)
        .border_1()
        .border_color(theme().colors.border)
        .cursor(CursorStyle::OpenHand)
        .on_drag(
            TerminalPaneMove { pane: pane_id },
            move |_, _, _window, cx| {
                let _ = drag_entity.update(cx, |this, cx| {
                    this.workspace.terminal.drop_side.borrow_mut().take();
                    this.workspace.terminal.dragging_pane = Some(pane_id);
                    cx.notify();
                });
                cx.new(|_| EmptyView)
            },
        );
    strip
        .child(
            pill.with_animation(
                format!("terminal-handle-{pane_id}"),
                Animation::new(Duration::from_millis(150))
                    .with_easing(|value| 1.0 - (1.0 - value).powi(3)),
                |pill, progress| pill.opacity(progress).mt(px(-4.0 * (1.0 - progress))),
            ),
        )
        .into_any_element()
}

fn drop_side_for(
    position: gpui::Point<Pixels>,
    pane_id: EntityId,
    pane_bounds: &PaneBounds,
) -> Option<TerminalDropSide> {
    let bounds = pane_bounds.borrow().get(&pane_id).copied()?;
    Some(TerminalDropSide::for_point(
        (position.x - bounds.origin.x).as_f32(),
        (position.y - bounds.origin.y).as_f32(),
        bounds.size.width.as_f32(),
        bounds.size.height.as_f32(),
    ))
}

fn terminal_drop_zone(
    pane_id: EntityId,
    entity: &WeakEntity<FarcasterApp>,
    pane_bounds: &PaneBounds,
) -> AnyElement {
    let hover_entity = entity.clone();
    let hover_bounds = Rc::clone(pane_bounds);
    let drop_entity = entity.clone();
    let drop_bounds = Rc::clone(pane_bounds);
    div()
        .id(format!("terminal-drop-{pane_id}"))
        .absolute()
        .left(px(0.0))
        .top(px(0.0))
        .right(px(0.0))
        .bottom(px(0.0))
        .on_mouse_move(move |event, _window, cx| {
            if !cx.has_active_drag() {
                return;
            }
            let position = event.position;
            let _ = hover_entity.update(cx, |this, cx| {
                let side = if this.workspace.terminal.dragging_pane == Some(pane_id) {
                    None
                } else {
                    drop_side_for(position, pane_id, &hover_bounds)
                };
                let value = side.map(|side| (pane_id, side));
                let mut cell = this.workspace.terminal.drop_side.borrow_mut();
                if *cell != value {
                    *cell = value;
                    drop(cell);
                    cx.notify();
                }
            });
        })
        .on_drop(move |payload: &TerminalPaneMove, window, cx| {
            cx.stop_propagation();
            let moved = payload.pane;
            let side = drop_side_for(window.mouse_position(), pane_id, &drop_bounds)
                .unwrap_or(TerminalDropSide::Right);
            let _ = drop_entity.update(cx, |this, cx| {
                this.workspace.terminal.dragging_pane = None;
                this.move_terminal_pane(moved, pane_id, side, window, cx);
            });
        })
        .into_any_element()
}

struct PaneBoundsProbe {
    pane_id: EntityId,
    bounds: PaneBounds,
}

impl IntoElement for PaneBoundsProbe {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for PaneBoundsProbe {
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
        let mut style = Style {
            position: Position::Absolute,
            ..Style::default()
        };
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        self.bounds.borrow_mut().insert(self.pane_id, bounds);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }
}

fn terminal_divider(
    row: bool,
    path: &[bool],
    hidden: bool,
    entity: &WeakEntity<FarcasterApp>,
) -> AnyElement {
    let color = theme().colors.separator;
    let line = if row {
        div().w(px(1.0)).h_full().bg(color)
    } else {
        div().h(px(1.0)).w_full().bg(color)
    };
    let clear_entity = entity.clone();
    let leave_entity = entity.clone();
    div()
        .id(format!("terminal-divider-{path:?}"))
        .flex_none()
        .when(row, |divider| divider.w(px(7.0)).py(px(6.0)))
        .when(!row, |divider| divider.h(px(7.0)).px(px(6.0)))
        .flex()
        .items_center()
        .justify_center()
        .cursor(if row {
            CursorStyle::ResizeLeftRight
        } else {
            CursorStyle::ResizeUpDown
        })
        .when(!hidden, |divider| divider.child(line))
        .on_mouse_move(move |_, _window, cx| {
            if !cx.has_active_drag() {
                return;
            }
            let _ = leave_entity.update(cx, |this, cx| {
                if this.workspace.terminal.dragging_pane.is_none() {
                    return;
                }
                let mut cell = this.workspace.terminal.drop_side.borrow_mut();
                if cell.is_some() {
                    *cell = None;
                    drop(cell);
                    cx.notify();
                }
            });
        })
        .on_drag(
            TerminalSplitResize {
                path: path.to_vec(),
            },
            move |_, _, _window, cx| {
                let _ = clear_entity.update(cx, |this, cx| {
                    this.workspace.terminal.drop_side.borrow_mut().take();
                    this.workspace.terminal.dragging_pane = None;
                    cx.notify();
                });
                cx.new(|_| EmptyView)
            },
        )
        .into_any_element()
}
