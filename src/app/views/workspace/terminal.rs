use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, App, AppContext as _, Bounds, CursorStyle, Element,
    ElementId, EmptyView, Entity, EntityId, GlobalElementId, InspectorElementId,
    InteractiveElement as _, IntoElement, LayoutId, MouseButton, ObjectFit, ParentElement as _,
    Pixels, Position, Rgba, RenderImage, StatefulInteractiveElement as _, Styled as _,
    StyledImage as _, Style, WeakEntity, Window, div, img, prelude::FluentBuilder as _, px,
    relative,
};
use gpui_libghostty::Terminal;

use crate::app::{
    FarcasterApp,
    ui::theme::theme,
    workspace::{
        HANDLE_BAND, HANDLE_PILL_HEIGHT, HANDLE_PILL_WIDTH, TerminalDropSide, TerminalLayout,
        TerminalPane, TerminalSplitDirection, handle_pill_bounds,
    },
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
            let overlay = overlay_sync(
                self.workspace.terminal.view.clone(),
                self.workspace.terminal.pane_bounds.clone(),
                None,
                None,
            );
            return div()
                .size_full()
                .min_h_0()
                .children(self.workspace.terminal.view.clone())
                .when_some(overlay, |root, overlay| root.child(overlay))
                .into_any_element();
        };
        let state = HandleState {
            hovered: self.workspace.terminal.hovered_handle,
            dragging: self.workspace.terminal.dragging_pane,
            drag_active,
            hide_borders: self.settings.hide_split_borders,
            handles: layout.leaf_count() > 1,
            drop_side: self.workspace.terminal.drop_side.clone(),
            pane_bounds: self.workspace.terminal.pane_bounds.clone(),
        };
        let preview = if state.drag_active && state.dragging.is_some() {
            *state.drop_side.borrow()
        } else {
            None
        };
        let pill = state.handles.then(|| {
            state
                .hovered
                .or(state.drag_active.then_some(state.dragging).flatten())
        });
        let overlay = overlay_sync(
            self.workspace.terminal.view.clone(),
            state.pane_bounds.clone(),
            preview,
            pill.flatten(),
        );
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
                None,
            ))
            .when_some(overlay, |root, overlay| root.child(overlay))
            .into_any_element()
    }

    pub(in crate::app::views) fn render_covered_terminal_workspace(
        &self,
        entity: WeakEntity<Self>,
    ) -> AnyElement {
        let Some(layout) = self.active_terminal_layout() else {
            return div()
                .size_full()
                .min_h_0()
                .when_some(
                    self.workspace.native_surface_snapshot.clone(),
                    |root, snapshot| {
                        root.child(img(snapshot).size_full().object_fit(ObjectFit::Fill))
                    },
                )
                .into_any_element();
        };
        let state = HandleState {
            hovered: None,
            dragging: None,
            drag_active: false,
            hide_borders: self.settings.hide_split_borders,
            handles: false,
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
                Some(&self.workspace.terminal_snapshots),
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
    snapshots: Option<&HashMap<EntityId, Arc<RenderImage>>>,
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
            let wrapper = match snapshots.and_then(|snapshots| snapshots.get(&pane_id)) {
                Some(snapshot) => div()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .child(img(snapshot.clone()).size_full().object_fit(ObjectFit::Fill)),
                None => div().flex_1().min_w_0().min_h_0().children(terminal),
            };
            let fill = |side: TerminalDropSide| {
                let fill = div().absolute().bg(theme().colors.drop_highlight);
                let fill = match side {
                    TerminalDropSide::Left => {
                        fill.left(px(0.0)).top(px(0.0)).w(relative(0.5)).h_full()
                    }
                    TerminalDropSide::Right => {
                        fill.right(px(0.0)).top(px(0.0)).w(relative(0.5)).h_full()
                    }
                    TerminalDropSide::Up => {
                        fill.left(px(0.0)).top(px(0.0)).w_full().h(relative(0.5))
                    }
                    TerminalDropSide::Down => {
                        fill.left(px(0.0)).bottom(px(0.0)).w_full().h(relative(0.5))
                    }
                };
                fill.with_animation(
                    format!("terminal-drop-fill-{pane_id}-{}", side.label()),
                    Animation::new(Duration::from_millis(120))
                        .with_easing(|value| value * value * (3.0 - 2.0 * value)),
                    |fill, progress| fill.opacity(progress),
                )
            };
            let pane = leaf
                .when_some(indicator, |pane, side| pane.child(fill(side)))
                .child(wrapper)
                .children(strip);
            let pane = pane.when(state.handles, |pane| {
                pane.child(PaneBoundsProbe {
                    pane_id,
                    bounds: Rc::clone(&state.pane_bounds),
                })
                .child(terminal_drop_zone(pane_id, entity, &state.pane_bounds))
            });
            pane.into_any_element()
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
                    snapshots,
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
                    snapshots,
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
        .absolute()
        .left(px(0.0))
        .top(px(0.0))
        .right(px(0.0))
        .h(px(HANDLE_BAND))
        .flex()
        .items_center()
        .justify_center()
        .block_mouse_except_scroll()
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
    let mut dots = div().flex().items_center().justify_center().gap(px(3.0));
    for _ in 0..3 {
        dots = dots.child(div().size(px(4.0)).rounded_full().bg(theme().colors.muted));
    }
    let pill = dots
        .id(format!("terminal-pane-handle-{pane_id}"))
        .w(px(HANDLE_PILL_WIDTH))
        .h(px(HANDLE_PILL_HEIGHT))
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

struct OverlayColors {
    preview: Rgba,
    background: Rgba,
    border: Rgba,
    dot: Rgba,
}

struct OverlaySync {
    terminal: Entity<Terminal>,
    bounds: PaneBounds,
    preview: Option<(EntityId, TerminalDropSide)>,
    pill: Option<EntityId>,
    colors: OverlayColors,
}

fn overlay_sync(
    terminal: Option<Entity<Terminal>>,
    bounds: PaneBounds,
    preview: Option<(EntityId, TerminalDropSide)>,
    pill: Option<EntityId>,
) -> Option<OverlaySync> {
    terminal.map(|terminal| OverlaySync {
        terminal,
        bounds,
        preview,
        pill,
        colors: OverlayColors {
            preview: theme().colors.drop_highlight,
            background: theme().colors.surface,
            border: theme().colors.border,
            dot: theme().colors.muted,
        },
    })
}

impl IntoElement for OverlaySync {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for OverlaySync {
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
        style.size.width = relative(0.0).into();
        style.size.height = relative(0.0).into();
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
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        _window: &mut Window,
        cx: &mut App,
    ) {
        let preview = self
            .preview
            .and_then(|(pane_id, side)| {
                self.bounds
                    .borrow()
                    .get(&pane_id)
                    .copied()
                    .map(|bounds| side.drop_bounds(bounds))
            })
            .unwrap_or_default();
        let pill = self
            .pill
            .and_then(|pane_id| {
                self.bounds
                    .borrow()
                    .get(&pane_id)
                    .copied()
                    .map(handle_pill_bounds)
            })
            .unwrap_or_default();
        let terminal = self.terminal.read(cx);
        terminal.overlay_preview(preview, self.colors.preview);
        terminal.overlay_pill(
            pill,
            self.colors.background,
            self.colors.border,
            self.colors.dot,
        );
    }
}
