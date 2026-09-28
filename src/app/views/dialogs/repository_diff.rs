use gpui::{
    AnyElement, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, WeakEntity, div, prelude::FluentBuilder as _, px,
};

use crate::{
    app::{
        FarcasterApp, OVERLAY_KEY_CONTEXT,
        ui::{
            assets::AppIcon,
            primitives::{
                AppIconSize, ButtonTone, app_icon, button, icon_button, icon_control, modal,
            },
            theme::{MONO_FONT_FAMILY, theme},
        },
    },
    repository::{DiffHunk, DiffLine, DiffLineKind, HunkApply, SplitRow},
};

const LINE_HEIGHT: f32 = 18.0;
const NUMBER_WIDTH: f32 = 44.0;

pub(in crate::app::views) fn render(
    app: &FarcasterApp,
    entity: WeakEntity<FarcasterApp>,
) -> Option<AnyElement> {
    let diff = app.overlays.repository_diff.as_ref()?;
    let close = entity.clone();
    let actions = diff.actions().to_vec();
    let available = app.project.repository.execution_allowed
        && app.project.repository.sync.action.is_none()
        && diff.applying.is_none()
        && diff.diff.is_some();
    let staged = diff.layer == crate::repository::ChangeLayer::Index;
    let path = diff.path.display().to_string();
    let additions = diff.additions;
    let deletions = diff.deletions;
    let hunks = diff
        .diff
        .as_ref()
        .map(|diff| diff.hunks.clone())
        .unwrap_or_default();
    let split = diff.split;
    let error = diff.error.clone();
    let preparing = diff.preparing();
    let applying = diff.applying;
    Some(
        modal(
            "repository-diff",
            "File diff",
            &app.overlays.repository_diff_focus,
            OVERLAY_KEY_CONTEXT,
            move |window, cx| {
                let _ = close.update(cx, |this, cx| this.close_repository_diff(window, cx));
            },
            |surface| {
                surface
                    .w(theme().size(980.0))
                    .max_w_full()
                    .h(gpui::relative(0.9))
                    .max_h(gpui::relative(0.94))
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .child(header(&entity, &path, staged, split, additions, deletions))
                    .child(
                        div()
                            .id("repository-diff-body")
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .overflow_y_scroll()
                            .bg(theme().colors.canvas)
                            .font_family(MONO_FONT_FAMILY)
                            .text_size(theme().type_scale.caption)
                            .when(preparing, |body| {
                                body.child(
                                    div()
                                        .p(theme().space.md)
                                        .text_color(theme().colors.subtle)
                                        .child("Reading the file diff…"),
                                )
                            })
                            .when(!preparing && hunks.is_empty() && error.is_none(), |body| {
                                body.child(
                                    div()
                                        .p(theme().space.md)
                                        .text_color(theme().colors.subtle)
                                        .child("No line changes to show"),
                                )
                            })
                            .children(hunks.iter().enumerate().map(|(index, hunk)| {
                                let entity = entity.clone();
                                let actions = actions.clone();
                                let body = if split {
                                    hunk.split_rows()
                                        .into_iter()
                                        .map(|row| split_row(hunk, row))
                                        .collect::<Vec<_>>()
                                } else {
                                    hunk.lines.iter().map(diff_line).collect::<Vec<_>>()
                                };
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(hunk_bar(
                                        index,
                                        &actions,
                                        available && applying.is_none(),
                                        entity,
                                    ))
                                    .children(body)
                                    .into_any_element()
                            })),
                    )
                    .when_some(error, |overlay, error| {
                        overlay.child(
                            div()
                                .flex_none()
                                .px(theme().space.md)
                                .py(theme().space.xs)
                                .text_size(theme().type_scale.caption)
                                .text_color(theme().colors.error)
                                .child(error),
                        )
                    })
            },
        )
        .into_any_element(),
    )
}

fn header(
    entity: &WeakEntity<FarcasterApp>,
    path: &str,
    staged: bool,
    split: bool,
    additions: u64,
    deletions: u64,
) -> AnyElement {
    let close = entity.clone();
    let reload = entity.clone();
    let layout = entity.clone();
    div()
        .flex_none()
        .h(theme().size(48.0))
        .px(theme().space.md)
        .flex()
        .items_center()
        .gap(theme().space.sm)
        .border_b(theme().border)
        .border_color(theme().colors.border)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_ellipsis()
                .font_family(MONO_FONT_FAMILY)
                .child(path.to_owned()),
        )
        .when(staged, |row| {
            row.child(
                div()
                    .flex_none()
                    .text_size(theme().type_scale.caption)
                    .text_color(theme().colors.subtle)
                    .child("Staged"),
            )
        })
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap(theme().space.xs)
                .text_size(theme().type_scale.caption)
                .child(
                    div()
                        .text_color(theme().colors.success)
                        .child(format!("+{additions}")),
                )
                .child(
                    div()
                        .text_color(theme().colors.error)
                        .child(format!("−{deletions}")),
                ),
        )
        .child(button(
            "toggle-repository-diff-layout",
            if split { "Inline" } else { "Side by side" },
            ButtonTone::Quiet,
            true,
            move |_, cx| {
                let _ = layout.update(cx, |this, cx| this.toggle_repository_diff_split(cx));
            },
        ))
        .child(icon_button(
            "reload-repository-diff",
            AppIcon::ArrowsClockwise,
            "Reload diff",
            ButtonTone::Quiet,
            move |_, cx| {
                let _ = reload.update(cx, |this, cx| this.reload_repository_diff(cx));
            },
        ))
        .child(icon_button(
            "close-repository-diff",
            AppIcon::X,
            "Close diff",
            ButtonTone::Quiet,
            move |window, cx| {
                let _ = close.update(cx, |this, cx| this.close_repository_diff(window, cx));
            },
        ))
        .into_any_element()
}

/// The rule between two blocks of changes.
///
/// VS Code's inline diff has no heading here either: the block boundary is a
/// line, and the per-block actions appear on it. They stay on screen at rest
/// rather than only on hover, so staging one block is findable without
/// guessing. `@@ -a,b +c,d @@` and its line numbers stay in the parser, which
/// needs them to rebuild a patch for one block.
fn hunk_bar(
    index: usize,
    actions: &[HunkApply],
    available: bool,
    entity: WeakEntity<FarcasterApp>,
) -> AnyElement {
    let group: SharedString = format!("repository-hunk-{index}").into();
    div()
        .id(("repository-hunk", index))
        .group(group.clone())
        .flex()
        .items_center()
        .justify_end()
        .gap(theme().space.xs)
        .h(theme().size(LINE_HEIGHT + 4.0))
        .px(theme().space.xs)
        .bg(theme().colors.canvas)
        .border_t(theme().border)
        .border_color(theme().colors.border)
        .hover(|bar| bar.bg(theme().colors.surface))
        .children(actions.iter().map(|mode| {
            let mode = *mode;
            let entity = entity.clone();
            hunk_action(index, mode, group.clone(), available, entity)
        }))
        .into_any_element()
}

fn hunk_action(
    index: usize,
    mode: HunkApply,
    group: SharedString,
    available: bool,
    entity: WeakEntity<FarcasterApp>,
) -> AnyElement {
    let label = mode.label();
    let control = icon_control(("repository-hunk-action", index * 3 + mode as usize), label)
        .size(theme().size(LINE_HEIGHT + 2.0))
        .child(app_icon(hunk_icon(mode), AppIconSize::Inline));
    let control = if available {
        control
            .opacity(0.55)
            .hover(|control| control.opacity(1.0))
            .group_hover(group, |control| control.opacity(1.0))
            .focus_visible(|control| control.opacity(1.0))
    } else {
        // The action stays visible but inert when the section cannot take it.
        control.opacity(0.25)
    };
    control
        .on_click(move |_, _, cx| {
            if available {
                cx.stop_propagation();
                let _ = entity.update(cx, |this, cx| this.apply_repository_hunk(index, mode, cx));
            }
        })
        .into_any_element()
}

const fn hunk_icon(mode: HunkApply) -> AppIcon {
    match mode {
        HunkApply::Stage => AppIcon::Plus,
        HunkApply::Unstage => AppIcon::Minus,
        HunkApply::Revert => AppIcon::ArrowCounterClockwise,
    }
}

/// One row of the side-by-side reading: the old file on the left, the new file
/// on the right, each half showing its own line number.
fn split_row(hunk: &DiffHunk, row: SplitRow) -> AnyElement {
    match row {
        SplitRow::Note { .. } => div()
            .px(theme().space.xs)
            .text_color(theme().colors.subtle)
            .child("\\ No newline at end of file")
            .into_any_element(),
        SplitRow::Pair { left, right } => div()
            .flex()
            .items_start()
            .child(split_side(hunk, left, Side::Old))
            .child(
                div()
                    .flex_none()
                    .w(theme().border)
                    .self_stretch()
                    .bg(theme().colors.border),
            )
            .child(split_side(hunk, right, Side::New))
            .into_any_element(),
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Side {
    Old,
    New,
}

fn split_side(hunk: &DiffHunk, index: Option<usize>, side: Side) -> AnyElement {
    let line = index.map(|index| &hunk.lines[index]);
    let (sign, tint) = match line.map(|line| line.kind) {
        Some(DiffLineKind::Added) => ("+", Some(theme().colors.success)),
        Some(DiffLineKind::Removed) => ("−", Some(theme().colors.error)),
        _ => ("", None),
    };
    let number = line.and_then(|line| match side {
        Side::Old => line.old_line,
        Side::New => line.new_line,
    });
    let text = line.map_or_else(String::new, |line| line.text.replace('\t', "    "));
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .items_start()
        .when_some(tint, |cell, tint| cell.bg(tint.opacity(0.20)))
        .child(number_cell(number))
        .child(
            div()
                .flex_none()
                .w(theme().size(14.0))
                .text_align(gpui::TextAlign::Center)
                .when_some(tint, |slot, tint| slot.text_color(tint))
                .child(sign.to_owned()),
        )
        .child(
            div()
                .min_w_0()
                .flex_1()
                .overflow_hidden()
                .whitespace_nowrap()
                .child(text),
        )
        .into_any_element()
}

fn number_cell(value: Option<u64>) -> AnyElement {
    div()
        .flex_none()
        .w(theme().size(NUMBER_WIDTH))
        .px(px(4.0))
        .text_align(gpui::TextAlign::Right)
        .text_color(theme().colors.subtle)
        .child(value.map_or_else(String::new, |value| value.to_string()))
        .into_any_element()
}

fn diff_line(line: &DiffLine) -> AnyElement {
    let (sign, tint) = match line.kind {
        DiffLineKind::Added => ("+", Some(theme().colors.success)),
        DiffLineKind::Removed => ("−", Some(theme().colors.error)),
        DiffLineKind::Context | DiffLineKind::Marker => ("", None),
    };
    let text = if line.kind == DiffLineKind::Marker {
        "\\ No newline at end of file".to_owned()
    } else {
        line.text.replace('\t', "    ")
    };
    div()
        .flex()
        .items_start()
        .when_some(tint, |row, tint| row.bg(tint.opacity(0.20)))
        .when(line.kind == DiffLineKind::Marker, |row| {
            row.text_color(theme().colors.subtle)
        })
        .child(number(line.old_line))
        .child(number(line.new_line))
        .child(
            div()
                .flex_none()
                .w(theme().size(14.0))
                .text_align(gpui::TextAlign::Center)
                .when_some(tint, |slot, tint| slot.text_color(tint))
                .child(sign.to_owned()),
        )
        .child(div().min_w_0().flex_1().whitespace_nowrap().child(text))
        .into_any_element()
}

fn number(value: Option<u64>) -> AnyElement {
    div()
        .flex_none()
        .w(theme().size(NUMBER_WIDTH))
        .px(px(4.0))
        .text_align(gpui::TextAlign::Right)
        .text_color(theme().colors.subtle)
        .child(value.map_or_else(String::new, |value| value.to_string()))
        .into_any_element()
}
