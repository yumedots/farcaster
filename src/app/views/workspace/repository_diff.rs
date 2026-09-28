use gpui::{
    AnyElement, InteractiveElement as _, IntoElement, ParentElement as _, PathBuilder, Pixels,
    SharedString, StatefulInteractiveElement as _, Styled as _, WeakEntity, canvas, div, point,
    prelude::FluentBuilder as _, px,
};

use crate::{
    app::{
        FarcasterApp, RepositoryDiff, diff_text,
        ui::{
            assets::AppIcon,
            primitives::{
                AppIconSize, AppTooltip as _, ButtonTone, app_icon, button, icon_button,
                icon_control,
            },
            theme::{MONO_FONT_FAMILY, theme},
        },
    },
    repository::{DiffHunk, DiffLine, DiffLineKind, DiffRow, DiffSource, HunkApply, SplitRow},
};

const LINE_HEIGHT: f32 = 18.0;
const OVERSCAN_ROWS: f32 = 8.0;
const NUMBER_WIDTH: f32 = 44.0;
const SIGN_WIDTH: f32 = 14.0;

fn number_width() -> Pixels {
    theme().size(NUMBER_WIDTH)
}

fn sign_width() -> Pixels {
    theme().size(SIGN_WIDTH)
}

fn half_gutter_width() -> Pixels {
    number_width() + sign_width()
}

fn gutter_width() -> Pixels {
    number_width() * 2.0 + sign_width()
}

pub(in crate::app::views) fn render(
    app: &FarcasterApp,
    entity: WeakEntity<FarcasterApp>,
) -> Option<AnyElement> {
    let diff = app.active_diff()?;
    let focus = app.overlays.repository_diff_focus.clone();
    let available = app.project.repository.execution_allowed
        && app.project.repository.sync.action.is_none()
        && diff.applying.is_none()
        && diff.diff.is_some();
    let staged = diff.layer == crate::repository::ChangeLayer::Index;
    let path = diff.path.display().to_string();
    let new_file = diff
        .diff
        .as_ref()
        .is_some_and(crate::repository::FileDiff::is_new_file);
    let error = diff.error.clone();
    Some(
        div()
            .id("repository-diff")
            .track_focus(&focus)
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(theme().colors.canvas)
            .child(header(
                &entity,
                &path,
                staged,
                diff.split,
                new_file,
                diff.hide_unchanged,
                diff.additions,
                diff.deletions,
            ))
            .child(body(diff, entity, available))
            .when_some(error, |diff, error| {
                diff.child(
                    div()
                        .flex_none()
                        .px(theme().space.md)
                        .py(theme().space.xs)
                        .text_size(theme().type_scale.caption)
                        .text_color(theme().colors.error)
                        .child(error),
                )
            })
            .into_any_element(),
    )
}

fn body(diff: &RepositoryDiff, entity: WeakEntity<FarcasterApp>, available: bool) -> AnyElement {
    let line_height = theme().size(LINE_HEIGHT);
    let height = |row: DiffRow| match row {
        DiffRow::Block { .. } => px(0.0),
        DiffRow::Line { .. } | DiffRow::Split { .. } | DiffRow::Band { .. } => line_height,
    };
    let rows = &diff.rows;
    let total = rows.iter().map(|row| f32::from(height(*row))).sum::<f32>();
    let scrolled = f32::from(diff.scroll.offset().x).max(0.0);
    let offset = f32::from(diff.scroll.offset().y).max(0.0);
    let bounds = diff.scroll.bounds();
    let viewport = f32::from(bounds.size.height);
    let window_width = px(f32::from(bounds.size.width));
    let reading_width = if diff.split_reading() {
        (half_gutter_width() + diff.widest_line) * 2.0 + theme().border
    } else {
        gutter_width() + diff.widest_line
    }
    .max(window_width);
    let half_width = px(f32::from(reading_width) / 2.0);
    let tail = (reading_width - window_width - px(scrolled)).max(px(0.0));
    let (first, first_top) = {
        let mut first = 0;
        let mut top = 0.0;
        while first < rows.len() {
            let next = top + f32::from(height(rows[first]));
            if next > offset {
                break;
            }
            top = next;
            first += 1;
        }
        (first, top)
    };
    let until = if viewport > 0.0 {
        offset + viewport + f32::from(line_height) * OVERSCAN_ROWS
    } else {
        f32::INFINITY
    };
    let (last, bottom) = {
        let mut last = first;
        let mut bottom = first_top;
        while last < rows.len() && bottom < until {
            bottom += f32::from(height(rows[last]));
            last += 1;
        }
        (last, bottom)
    };
    let mut top = first_top;
    let mut grouped: Vec<(Option<usize>, Vec<AnyElement>)> = Vec::new();
    let mut anchors = Vec::new();
    for row in &rows[first..last] {
        let row_top = top;
        top += f32::from(height(*row));
        if let DiffRow::Block { hunk } = *row {
            anchors.push((hunk, row_top));
            continue;
        }
        let element = diff_row(diff, *row, entity.clone(), line_height, row_top, half_width);
        let hunk = row_hunk(*row);
        match grouped.last_mut() {
            Some((owner, elements)) if *owner == hunk => elements.push(element),
            _ => grouped.push((hunk, vec![element])),
        }
    }
    let drawn = grouped
        .into_iter()
        .map(|(hunk, elements)| {
            let group = div().flex_none().flex().flex_col().children(elements);
            match hunk {
                Some(hunk) => group.group(hunk_group(hunk)).into_any_element(),
                None => group.into_any_element(),
            }
        })
        .collect::<Vec<_>>();
    let floating = available.then(|| {
        anchors
            .into_iter()
            .map(|(hunk, row_top)| {
                block_actions(
                    hunk,
                    diff.actions(),
                    entity.clone(),
                    tail,
                    row_top,
                    line_height + theme().size(2.0),
                )
            })
            .collect::<Vec<_>>()
    });
    let preparing = diff.preparing();
    let empty = diff.diff.as_ref().is_none_or(|file| file.hunks.is_empty());
    let reading = div()
        .flex()
        .flex_col()
        .relative()
        .min_w(reading_width)
        .when(first_top > 0.0, |reading| {
            reading.child(spacer(px(first_top)))
        })
        .children(drawn)
        .when(total - bottom > 0.0, |reading| {
            reading.child(spacer(px(total - bottom)))
        })
        .children(floating.into_iter().flatten());
    div()
        .id("repository-diff-body")
        .track_scroll(&diff.scroll)
        .flex_1()
        .min_h_0()
        .min_w_0()
        .overflow_scroll()
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
        .when(!preparing && empty, |body| {
            body.child(
                div()
                    .p(theme().space.md)
                    .text_color(theme().colors.subtle)
                    .child("No line changes to show"),
            )
        })
        .when(!preparing && !empty, |body| body.child(reading))
        .into_any_element()
}

const fn row_hunk(row: DiffRow) -> Option<usize> {
    match row {
        DiffRow::Block { hunk } => Some(hunk),
        DiffRow::Line { source, .. } | DiffRow::Split { source, .. } => match source {
            DiffSource::Hunk(hunk) => Some(hunk),
            DiffSource::Unchanged(_) => None,
        },
        DiffRow::Band { .. } => None,
    }
}

fn spacer(height: Pixels) -> AnyElement {
    div().flex_none().w_full().h(height).into_any_element()
}

fn diff_row(
    diff: &RepositoryDiff,
    row: DiffRow,
    entity: WeakEntity<FarcasterApp>,
    line_height: Pixels,
    top: f32,
    half_width: Pixels,
) -> AnyElement {
    let Some(file) = diff.diff.as_ref() else {
        return div().into_any_element();
    };
    match row {
        DiffRow::Block { .. } => div().into_any_element(),
        DiffRow::Line { source, line } => match source {
            DiffSource::Hunk(hunk) => diff_line(&file.hunks[hunk].lines[line], line_height, top),
            DiffSource::Unchanged(span) => span_line(diff, span, line, line_height, top),
        },
        DiffRow::Split { source, row } => match source {
            DiffSource::Hunk(hunk) => {
                split_row(&file.hunks[hunk], row, line_height, half_width, top)
            }
            DiffSource::Unchanged(span) => match span_lines(diff, span).get(row_line(row)) {
                Some(line) => unchanged_row(line, line_height, half_width, top),
                None => div().h(line_height).into_any_element(),
            },
        },
        DiffRow::Band {
            span,
            lines,
            folded,
        } => unchanged_band(span, lines, folded, entity, line_height),
    }
}

fn span_lines(diff: &RepositoryDiff, span: usize) -> &[DiffLine] {
    diff.diff
        .as_ref()
        .and_then(|file| file.spans().get(span))
        .map_or(&[], Vec::as_slice)
}

const fn row_line(row: SplitRow) -> usize {
    match row {
        SplitRow::Pair { left, right } => match right {
            Some(line) => line,
            None => match left {
                Some(line) => line,
                None => 0,
            },
        },
        SplitRow::Note { line } => line,
    }
}

fn span_line(
    diff: &RepositoryDiff,
    span: usize,
    line: usize,
    height: Pixels,
    origin: f32,
) -> AnyElement {
    match span_lines(diff, span).get(line) {
        Some(line) => diff_line(line, height, origin),
        None => div().h(height).into_any_element(),
    }
}

fn unchanged_row(line: &DiffLine, height: Pixels, half_width: Pixels, origin: f32) -> AnyElement {
    let text = diff_text(line.drawn_text());
    let side = |number: Option<u64>| {
        div()
            .flex_1()
            .min_w(half_width)
            .h(height)
            .flex()
            .items_start()
            .child(number_cell(number))
            .child(div().flex_none().whitespace_nowrap().child(text.clone()))
    };
    div()
        .relative()
        .flex()
        .items_start()
        .h(height)
        .child(hatch(origin, unchanged_ink()))
        .child(side(line.old_line))
        .child(
            div()
                .flex_none()
                .w(theme().border)
                .self_stretch()
                .bg(theme().colors.border),
        )
        .child(side(line.new_line))
        .into_any_element()
}

fn unchanged_band(
    span: usize,
    lines: usize,
    folded: bool,
    entity: WeakEntity<FarcasterApp>,
    height: Pixels,
) -> AnyElement {
    let label = if lines == 1 {
        "1 unchanged line".to_owned()
    } else {
        format!("{lines} unchanged lines")
    };
    let hint: SharedString = if folded {
        "Show these lines".into()
    } else {
        "Hide these lines".into()
    };
    div()
        .id(("repository-diff-band", span))
        .role(gpui::Role::Button)
        .aria_label(label.clone())
        .tab_index(0)
        .flex()
        .items_center()
        .h(height)
        .cursor_pointer()
        .text_color(theme().colors.subtle)
        .hover(|band| band.bg(theme().colors.highlight))
        .app_tooltip(hint)
        .child(
            div()
                .flex_none()
                .w(number_width())
                .px(px(4.0))
                .flex()
                .justify_end()
                .child(app_icon(
                    if folded {
                        AppIcon::CaretUp
                    } else {
                        AppIcon::CaretDown
                    },
                    AppIconSize::Inline,
                )),
        )
        .child(div().flex_none().w(half_gutter_width()))
        .child(label)
        .on_click(move |_, _, cx| {
            let _ = entity.update(cx, |this, cx| {
                this.toggle_repository_diff_span(span, cx);
            });
        })
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn header(
    entity: &WeakEntity<FarcasterApp>,
    path: &str,
    staged: bool,
    split: bool,
    new_file: bool,
    hide_unchanged: bool,
    additions: u64,
    deletions: u64,
) -> AnyElement {
    let reload = entity.clone();
    let layout = entity.clone();
    let unchanged = entity.clone();
    div()
        .flex_none()
        .h(theme().size(34.0))
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
                .text_size(theme().type_scale.caption)
                .text_color(theme().colors.muted)
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
        .when(!new_file, |row| {
            row.child(button(
                "toggle-repository-diff-layout",
                if split { "Inline" } else { "Side by side" },
                ButtonTone::Quiet,
                true,
                move |_, cx| {
                    let _ = layout.update(cx, |this, cx| this.toggle_repository_diff_split(cx));
                },
            ))
        })
        .child(button(
            "toggle-repository-diff-unchanged",
            if hide_unchanged {
                "Show unchanged"
            } else {
                "Hide unchanged"
            },
            ButtonTone::Quiet,
            true,
            move |_, cx| {
                let _ =
                    unchanged.update(cx, |this, cx| this.toggle_settings_hide_unchanged_lines(cx));
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
        .into_any_element()
}

fn block_actions(
    hunk: usize,
    actions: &[HunkApply],
    entity: WeakEntity<FarcasterApp>,
    tail: Pixels,
    top: f32,
    height: Pixels,
) -> AnyElement {
    div()
        .absolute()
        .top(px(top))
        .right(tail + theme().space.xs)
        .tab_index(0)
        .flex()
        .items_center()
        .gap(theme().space.xs)
        .px(theme().space.xs)
        .h(height)
        .rounded(theme().radius)
        .bg(theme().colors.surface)
        .border(theme().border)
        .border_color(theme().colors.border)
        .opacity(0.0)
        .hover(|widget| widget.opacity(1.0))
        .group_hover(hunk_group(hunk), |widget| widget.opacity(1.0))
        .focus_visible(|widget| widget.opacity(1.0))
        .children(actions.iter().map(|mode| {
            let mode = *mode;
            hunk_action(hunk, mode, entity.clone())
        }))
        .into_any_element()
}

fn hunk_action(index: usize, mode: HunkApply, entity: WeakEntity<FarcasterApp>) -> AnyElement {
    icon_control(
        ("repository-hunk-action", index * 3 + mode as usize),
        mode.label(),
    )
    .size(theme().size(LINE_HEIGHT))
    .child(app_icon(hunk_icon(mode), AppIconSize::Inline))
    .on_click(move |_, _, cx| {
        cx.stop_propagation();
        let _ = entity.update(cx, |this, cx| this.apply_repository_hunk(index, mode, cx));
    })
    .into_any_element()
}

fn hunk_group(hunk: usize) -> SharedString {
    SharedString::from(format!("repository-diff-hunk-{hunk}"))
}

const fn hunk_icon(mode: HunkApply) -> AppIcon {
    match mode {
        HunkApply::Stage => AppIcon::Plus,
        HunkApply::Unstage => AppIcon::Minus,
        HunkApply::Revert => AppIcon::ArrowCounterClockwise,
    }
}

fn split_row(
    hunk: &DiffHunk,
    row: SplitRow,
    height: Pixels,
    half_width: Pixels,
    top: f32,
) -> AnyElement {
    match row {
        SplitRow::Note { line } => div()
            .h(height)
            .px(theme().space.xs)
            .text_color(theme().colors.subtle)
            .child(hunk.lines[line].drawn_text().to_owned())
            .into_any_element(),
        SplitRow::Pair { left, right } => {
            let unchanged = left
                .filter(|line| Some(*line) == right)
                .is_some_and(|line| hunk.lines[line].kind == DiffLineKind::Context);
            div()
                .relative()
                .flex()
                .items_start()
                .h(height)
                .when(unchanged, |line| line.child(hatch(top, unchanged_ink())))
                .child(split_side(hunk, left, Side::Old, height, half_width, top))
                .child(
                    div()
                        .flex_none()
                        .w(theme().border)
                        .self_stretch()
                        .bg(theme().colors.border),
                )
                .child(split_side(hunk, right, Side::New, height, half_width, top))
                .into_any_element()
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Side {
    Old,
    New,
}

impl Side {
    const fn selector(self) -> &'static str {
        match self {
            Self::Old => "repository-diff-old-text",
            Self::New => "repository-diff-new-text",
        }
    }
}

fn split_side(
    hunk: &DiffHunk,
    index: Option<usize>,
    side: Side,
    height: Pixels,
    half_width: Pixels,
    top: f32,
) -> AnyElement {
    let Some(line) = index.map(|index| &hunk.lines[index]) else {
        let origin = match side {
            Side::Old => top,
            Side::New => top + f32::from(half_width) + f32::from(theme().border),
        };
        return empty_side(height, half_width, origin);
    };
    let (sign, tint) = match line.kind {
        DiffLineKind::Added => ("+", Some(theme().colors.success)),
        DiffLineKind::Removed => ("−", Some(theme().colors.error)),
        _ => ("", None),
    };
    let number = match side {
        Side::Old => line.old_line,
        Side::New => line.new_line,
    };
    let text = diff_text(&line.text);
    div()
        .flex_1()
        .min_w(half_width)
        .h(height)
        .flex()
        .items_start()
        .when_some(tint, |cell, tint| cell.bg(tint.opacity(0.20)))
        .child(number_cell(number))
        .child(
            div()
                .flex_none()
                .w(sign_width())
                .text_align(gpui::TextAlign::Center)
                .when_some(tint, |slot, tint| slot.text_color(tint))
                .child(sign.to_owned()),
        )
        .child(
            div()
                .debug_selector(move || side.selector().into())
                .flex_none()
                .whitespace_nowrap()
                .child(text),
        )
        .into_any_element()
}

fn empty_side(height: Pixels, half_width: Pixels, origin: f32) -> AnyElement {
    div()
        .relative()
        .flex_1()
        .min_w(half_width)
        .h(height)
        .overflow_hidden()
        .child(hatch(origin, empty_ink()))
        .into_any_element()
}

fn empty_ink() -> gpui::Hsla {
    theme().colors.border.opacity(0.45).into()
}

fn unchanged_ink() -> gpui::Hsla {
    theme().colors.border.opacity(0.14).into()
}

fn hatch(origin: f32, ink: gpui::Hsla) -> AnyElement {
    canvas(
        |bounds, _, _| bounds,
        move |bounds, _, window, _| {
            let width = f32::from(bounds.size.width);
            let height = f32::from(bounds.size.height);
            let step = f32::from(theme().size(8.0)).max(2.0) * std::f32::consts::SQRT_2;
            let mut builder = PathBuilder::stroke(px(1.0));
            let mut offset = -(origin % step);
            while offset < width + height {
                let start_x = (offset - height).max(0.0);
                let end_x = offset.min(width);
                if end_x > start_x {
                    builder.add_polygon(
                        &[
                            point(
                                bounds.origin.x + px(start_x),
                                bounds.origin.y + px(offset - start_x),
                            ),
                            point(
                                bounds.origin.x + px(end_x),
                                bounds.origin.y + px(offset - end_x),
                            ),
                        ],
                        false,
                    );
                }
                offset += step;
            }
            if let Ok(path) = builder.build() {
                window.paint_path(path, ink);
            }
        },
    )
    .absolute()
    .size_full()
    .into_any_element()
}

fn number_cell(value: Option<u64>) -> AnyElement {
    div()
        .flex_none()
        .w(number_width())
        .px(px(4.0))
        .text_align(gpui::TextAlign::Right)
        .text_color(theme().colors.subtle)
        .child(value.map_or_else(String::new, |value| value.to_string()))
        .into_any_element()
}

fn diff_line(line: &DiffLine, height: Pixels, origin: f32) -> AnyElement {
    let (sign, tint) = match line.kind {
        DiffLineKind::Added => ("+", Some(theme().colors.success)),
        DiffLineKind::Removed => ("−", Some(theme().colors.error)),
        DiffLineKind::Context | DiffLineKind::Marker => ("", None),
    };
    let text = diff_text(line.drawn_text());
    let unchanged = line.kind == DiffLineKind::Context;
    div()
        .relative()
        .flex()
        .items_start()
        .h(height)
        .when(unchanged, |row| row.child(hatch(origin, unchanged_ink())))
        .when_some(tint, |row, tint| row.bg(tint.opacity(0.20)))
        .when(line.kind == DiffLineKind::Marker, |row| {
            row.text_color(theme().colors.subtle)
        })
        .child(number(line.old_line))
        .child(number(line.new_line))
        .child(
            div()
                .flex_none()
                .w(sign_width())
                .text_align(gpui::TextAlign::Center)
                .when_some(tint, |slot, tint| slot.text_color(tint))
                .child(sign.to_owned()),
        )
        .child(div().flex_none().whitespace_nowrap().child(text))
        .into_any_element()
}

fn number(value: Option<u64>) -> AnyElement {
    div()
        .flex_none()
        .w(number_width())
        .px(px(4.0))
        .text_align(gpui::TextAlign::Right)
        .text_color(theme().colors.subtle)
        .child(value.map_or_else(String::new, |value| value.to_string()))
        .into_any_element()
}
