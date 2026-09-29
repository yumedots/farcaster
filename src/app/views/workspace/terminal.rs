use gpui::{
    AnyElement, AppContext as _, CursorStyle, EmptyView, EntityId, InteractiveElement as _,
    IntoElement as _, MouseButton, ParentElement as _, StatefulInteractiveElement as _,
    Styled as _, WeakEntity, div, prelude::FluentBuilder as _, px,
};

use crate::app::{
    FarcasterApp,
    ui::theme::theme,
    workspace::{TerminalLayout, TerminalPane, TerminalSplitDirection},
};

struct TerminalSplitResize {
    path: Vec<bool>,
}

impl FarcasterApp {
    pub(in crate::app::views) fn render_terminal_workspace(
        &self,
        entity: WeakEntity<Self>,
    ) -> AnyElement {
        let Some(layout) = self.active_terminal_layout() else {
            return div()
                .size_full()
                .min_h_0()
                .children(self.workspace.terminal.view.clone())
                .into_any_element();
        };
        div()
            .size_full()
            .min_h_0()
            .flex()
            .child(terminal_pane_element(
                layout,
                layout.root(),
                layout.focused_id(),
                &entity,
                1.0,
                &[],
            ))
            .into_any_element()
    }
}

fn terminal_pane_element(
    layout: &TerminalLayout,
    pane: &TerminalPane,
    focused: EntityId,
    entity: &WeakEntity<FarcasterApp>,
    grow: f32,
    path: &[bool],
) -> AnyElement {
    match pane {
        TerminalPane::Leaf(id) => {
            let pane_id = *id;
            let terminal = layout.terminal(pane_id).cloned();
            let click = entity.clone();
            div()
                .id(format!("terminal-pane-{pane_id}"))
                .flex_grow(grow)
                .flex_shrink_1()
                .min_w_0()
                .min_h_0()
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    let _ =
                        click.update(cx, |this, cx| this.focus_terminal_pane(pane_id, window, cx));
                })
                .children(terminal)
                .into_any_element()
        }
        TerminalPane::Split {
            direction,
            ratio,
            first,
            second,
        } => {
            let row = *direction == TerminalSplitDirection::Right;
            let divider_active = TerminalLayout::subtree_contains(first, focused)
                || TerminalLayout::subtree_contains(second, focused);
            let mut first_path = path.to_vec();
            first_path.push(false);
            let mut second_path = path.to_vec();
            second_path.push(true);
            let drag_path = path.to_vec();
            let drag_entity = entity.clone();
            div()
                .flex_grow(grow)
                .flex_shrink_1()
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
                    focused,
                    entity,
                    *ratio,
                    &first_path,
                ))
                .child(terminal_divider(row, divider_active, path))
                .child(terminal_pane_element(
                    layout,
                    second,
                    focused,
                    entity,
                    1.0 - *ratio,
                    &second_path,
                ))
                .into_any_element()
        }
    }
}

fn terminal_divider(row: bool, active: bool, path: &[bool]) -> AnyElement {
    let color = if active {
        theme().colors.accent
    } else {
        theme().colors.border
    };
    let line = if row {
        div().w(px(1.0)).h_full().bg(color)
    } else {
        div().h(px(1.0)).w_full().bg(color)
    };
    div()
        .id(format!("terminal-divider-{path:?}"))
        .flex_none()
        .when(row, |divider| divider.w(px(7.0)))
        .when(!row, |divider| divider.h(px(7.0)))
        .flex()
        .items_center()
        .justify_center()
        .cursor(if row {
            CursorStyle::ResizeLeftRight
        } else {
            CursorStyle::ResizeUpDown
        })
        .child(line)
        .on_drag(
            TerminalSplitResize {
                path: path.to_vec(),
            },
            |_, _, _window, cx| cx.new(|_| EmptyView),
        )
        .into_any_element()
}
