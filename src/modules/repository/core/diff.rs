use std::{collections::BTreeMap, fmt::Write as _, ops::Range};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HunkApply {
    Stage,
    Unstage,
    Revert,
}

impl HunkApply {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Stage => "Stage hunk",
            Self::Unstage => "Unstage hunk",
            Self::Revert => "Revert hunk",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SideWidths {
    pub(crate) old: f32,
    pub(crate) new: f32,
}

pub(crate) const NO_NEWLINE_NOTE: &str = "\\ No newline at end of file";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DiffLineKind {
    Context,
    Added,
    Removed,
    Marker,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DiffLine {
    pub(crate) kind: DiffLineKind,
    pub(crate) text: String,
    pub(crate) old_line: Option<u64>,
    pub(crate) new_line: Option<u64>,
}

impl DiffLine {
    pub(crate) fn drawn_text(&self) -> &str {
        match self.kind {
            DiffLineKind::Marker => NO_NEWLINE_NOTE,
            _ => &self.text,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DiffHunk {
    pub(crate) heading: String,
    pub(crate) lines: Vec<DiffLine>,
    pub(crate) additions: usize,
    pub(crate) deletions: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DiffSegment {
    Unchanged(Range<usize>),
    Changed(Range<usize>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SplitRow {
    Note {
        line: usize,
    },
    Pair {
        left: Option<usize>,
        right: Option<usize>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DiffSource {
    Hunk(usize),
    Unchanged(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DiffRow {
    Block {
        hunk: usize,
    },
    Line {
        source: DiffSource,
        line: usize,
    },
    Split {
        source: DiffSource,
        row: SplitRow,
    },
    Band {
        span: usize,
        lines: usize,
        folded: bool,
    },
}

impl DiffHunk {
    pub(crate) fn segments(&self) -> Vec<DiffSegment> {
        let mut segments = Vec::new();
        let mut start = 0;
        let mut unchanged = !self
            .lines
            .first()
            .is_some_and(|line| line.kind != DiffLineKind::Context);
        while start < self.lines.len() {
            let mut end = start + 1;
            while end < self.lines.len() {
                let belongs = match self.lines[end].kind {
                    DiffLineKind::Context => true,
                    DiffLineKind::Marker => unchanged,
                    DiffLineKind::Added | DiffLineKind::Removed => false,
                };
                if belongs != unchanged {
                    break;
                }
                end += 1;
            }
            segments.push(if unchanged {
                DiffSegment::Unchanged(start..end)
            } else {
                DiffSegment::Changed(start..end)
            });
            start = end;
            unchanged = !unchanged;
        }
        segments
    }

    pub(crate) fn pair_segment(&self, range: Range<usize>) -> Vec<SplitRow> {
        let kind = |index: usize| self.lines[index].kind;
        let mut rows = Vec::new();
        let mut index = range.start;
        while index < range.end {
            match kind(index) {
                DiffLineKind::Marker => {
                    rows.push(SplitRow::Note { line: index });
                    index += 1;
                }
                DiffLineKind::Context => {
                    rows.push(SplitRow::Pair {
                        left: Some(index),
                        right: Some(index),
                    });
                    index += 1;
                }
                DiffLineKind::Added => {
                    rows.push(SplitRow::Pair {
                        left: None,
                        right: Some(index),
                    });
                    index += 1;
                }
                DiffLineKind::Removed => {
                    let removed = index;
                    while index < range.end && kind(index) == DiffLineKind::Removed {
                        index += 1;
                    }
                    let removed_len = index - removed;
                    let added = index;
                    while index < range.end && kind(index) == DiffLineKind::Added {
                        index += 1;
                    }
                    let added_len = index - added;
                    for offset in 0..removed_len.max(added_len) {
                        rows.push(SplitRow::Pair {
                            left: (offset < removed_len).then_some(removed + offset),
                            right: (offset < added_len).then_some(added + offset),
                        });
                    }
                }
            }
        }
        rows
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FileDiff {
    header: String,
    pub(crate) hunks: Vec<DiffHunk>,
    spans: Vec<Vec<DiffLine>>,
}

impl FileDiff {
    pub(crate) fn parse(patch: &str) -> Self {
        let lines = patch.lines().collect::<Vec<_>>();
        let mut header = String::new();
        let mut hunks = Vec::new();
        let mut index = 0;
        while index < lines.len() {
            let line = lines[index];
            if line.starts_with("@@ ") || line.starts_with("@@-") {
                let start = index + 1;
                index = start;
                while index < lines.len() && hunk_line(lines[index]).is_some() {
                    index += 1;
                }
                hunks.push(hunk(line, &lines[start..index]));
                continue;
            }
            if !hunks.is_empty() {
                break;
            }
            header.push_str(line);
            header.push('\n');
            index += 1;
        }
        let mut diff = Self {
            header,
            hunks,
            spans: Vec::new(),
        };
        diff.rebuild_spans(&[]);
        diff
    }

    pub(crate) fn is_new_file(&self) -> bool {
        self.header
            .lines()
            .any(|line| line == "--- /dev/null" || line == "--- NUL")
    }

    pub(crate) fn spans(&self) -> &[Vec<DiffLine>] {
        &self.spans
    }

    pub(crate) fn absorb_unchanged(&mut self, full: &Self) {
        let file = full
            .hunks
            .iter()
            .flat_map(|hunk| hunk.lines.iter())
            .filter_map(|line| line.new_line.map(|number| (number, line)))
            .collect::<BTreeMap<_, _>>();
        let between = |from: u64, to: u64| {
            file.range(from..to)
                .map(|(_, line)| (*line).clone())
                .collect::<Vec<_>>()
        };
        let mut gaps = Vec::with_capacity(self.hunks.len() + 1);
        let mut next = 1_u64;
        for hunk in &self.hunks {
            let first = hunk.lines.iter().filter_map(|line| line.new_line).min();
            let last = hunk.lines.iter().filter_map(|line| line.new_line).max();
            match (first, last) {
                (Some(first), Some(last)) => {
                    gaps.push(between(next, first));
                    next = last.saturating_add(1);
                }
                _ => gaps.push(Vec::new()),
            }
        }
        gaps.push(between(next, u64::MAX));
        self.rebuild_spans(&gaps);
    }

    fn rebuild_spans(&mut self, gaps: &[Vec<DiffLine>]) {
        let mut spans = Vec::with_capacity(self.hunks.len() + 1);
        let mut current = gaps.first().cloned().unwrap_or_default();
        for (index, hunk) in self.hunks.iter().enumerate() {
            for segment in hunk.segments() {
                match segment {
                    DiffSegment::Unchanged(range) => {
                        current.extend(hunk.lines[range].iter().cloned());
                    }
                    DiffSegment::Changed(_) => spans.push(std::mem::take(&mut current)),
                }
            }
            current.extend(gaps.get(index + 1).into_iter().flatten().cloned());
        }
        spans.push(current);
        self.spans = spans;
    }

    pub(crate) fn widest_sides(&self, measure: impl Fn(&str) -> f32) -> SideWidths {
        let mut widths = SideWidths { old: 0.0, new: 0.0 };
        for line in self.lines() {
            let width = measure(line.drawn_text());
            if line.kind != DiffLineKind::Added {
                widths.old = widths.old.max(width);
            }
            if line.kind != DiffLineKind::Removed {
                widths.new = widths.new.max(width);
            }
        }
        widths
    }

    fn lines(&self) -> impl Iterator<Item = &DiffLine> {
        self.hunks
            .iter()
            .flat_map(|hunk| hunk.lines.iter())
            .chain(self.spans.iter().flatten())
    }

    pub(crate) fn patch_for(&self, index: usize) -> Option<String> {
        let hunk = self.hunks.get(index)?;
        let mut patch = String::with_capacity(self.header.len() + hunk.heading.len() + 64);
        patch.push_str(&self.header);
        patch.push_str(&hunk.heading);
        patch.push('\n');
        for line in &hunk.lines {
            match line.kind {
                DiffLineKind::Marker => {
                    patch.push_str(NO_NEWLINE_NOTE);
                    patch.push('\n');
                }
                DiffLineKind::Context => {
                    let _ = writeln!(patch, " {}", line.text);
                }
                DiffLineKind::Added => {
                    let _ = writeln!(patch, "+{}", line.text);
                }
                DiffLineKind::Removed => {
                    let _ = writeln!(patch, "-{}", line.text);
                }
            }
        }
        Some(patch)
    }

    pub(crate) fn rows(&self, split: bool, hidden: bool, opened: &[usize]) -> Vec<DiffRow> {
        let split = split && !self.is_new_file();
        let mut rows = Vec::new();
        let mut span = 0;
        for (index, hunk) in self.hunks.iter().enumerate() {
            let mut block = false;
            for segment in hunk.segments() {
                let DiffSegment::Changed(range) = segment else {
                    continue;
                };
                self.span_rows(&mut rows, span, split, hidden, opened);
                span += 1;
                if !block {
                    rows.push(DiffRow::Block { hunk: index });
                    block = true;
                }
                if split {
                    rows.extend(
                        hunk.pair_segment(range)
                            .into_iter()
                            .map(|row| DiffRow::Split {
                                source: DiffSource::Hunk(index),
                                row,
                            }),
                    );
                } else {
                    rows.extend(range.map(|line| DiffRow::Line {
                        source: DiffSource::Hunk(index),
                        line,
                    }));
                }
            }
        }
        self.span_rows(&mut rows, span, split, hidden, opened);
        rows
    }

    fn span_rows(
        &self,
        rows: &mut Vec<DiffRow>,
        span: usize,
        split: bool,
        hidden: bool,
        opened: &[usize],
    ) {
        let Some(lines) = self.spans.get(span) else {
            return;
        };
        if lines.is_empty() {
            return;
        }
        let count = lines.len();
        if hidden && !opened.contains(&span) {
            rows.push(DiffRow::Band {
                span,
                lines: count,
                folded: true,
            });
            return;
        }
        if hidden {
            rows.push(DiffRow::Band {
                span,
                lines: count,
                folded: false,
            });
        }
        if split {
            rows.extend((0..lines.len()).map(|line| DiffRow::Split {
                source: DiffSource::Unchanged(span),
                row: SplitRow::Pair {
                    left: Some(line),
                    right: Some(line),
                },
            }));
        } else {
            rows.extend((0..lines.len()).map(|line| DiffRow::Line {
                source: DiffSource::Unchanged(span),
                line,
            }));
        }
    }
}

fn hunk_line(line: &str) -> Option<DiffLineKind> {
    match line.as_bytes().first() {
        Some(b' ') => Some(DiffLineKind::Context),
        Some(b'+') => Some(DiffLineKind::Added),
        Some(b'-') => Some(DiffLineKind::Removed),
        Some(b'\\') => Some(DiffLineKind::Marker),
        _ => None,
    }
}

fn hunk(heading: &str, lines: &[&str]) -> DiffHunk {
    let (mut old_line, mut new_line) = heading_lines(heading);
    let mut parsed = Vec::with_capacity(lines.len());
    let mut additions = 0;
    let mut deletions = 0;
    for line in lines {
        let Some(kind) = hunk_line(line) else {
            break;
        };
        let text = if kind == DiffLineKind::Marker {
            ""
        } else {
            &line[1..]
        };
        let (old, new) = match kind {
            DiffLineKind::Context => {
                let current = (Some(old_line), Some(new_line));
                old_line = old_line.saturating_add(1);
                new_line = new_line.saturating_add(1);
                current
            }
            DiffLineKind::Added => {
                let current = (None, Some(new_line));
                new_line = new_line.saturating_add(1);
                additions += 1;
                current
            }
            DiffLineKind::Removed => {
                let current = (Some(old_line), None);
                old_line = old_line.saturating_add(1);
                deletions += 1;
                current
            }
            DiffLineKind::Marker => (None, None),
        };
        parsed.push(DiffLine {
            kind,
            text: text.to_owned(),
            old_line: old,
            new_line: new,
        });
    }
    DiffHunk {
        heading: heading.to_owned(),
        lines: parsed,
        additions,
        deletions,
    }
}

fn heading_lines(heading: &str) -> (u64, u64) {
    let mut ranges = heading.split_whitespace();
    let old = ranges
        .by_ref()
        .find(|part| part.starts_with('-') && starts_with_digit(&part[1..]))
        .and_then(range_start);
    let new = ranges
        .find(|part| part.starts_with('+') && starts_with_digit(&part[1..]))
        .and_then(range_start);
    (old.unwrap_or(1), new.unwrap_or(1))
}

fn starts_with_digit(value: &str) -> bool {
    value.as_bytes().first().is_some_and(u8::is_ascii_digit)
}

fn range_start(range: &str) -> Option<u64> {
    range
        .split(',')
        .next()
        .map(|value| value.trim_start_matches(['-', '+']))
        .and_then(|value| value.parse().ok())
}

#[cfg(test)]
#[path = "diff_tests.rs"]
mod tests;
