use std::fmt::Write as _;

/// Which copy of a file a hunk is applied to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HunkApply {
    /// Stage a hunk from the working tree into the index.
    Stage,
    /// Take a hunk back out of the index.
    Unstage,
    /// Throw away a hunk in the working tree.
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DiffLineKind {
    Context,
    Added,
    Removed,
    /// A `\ No newline at end of file` marker that belongs to the line above it.
    Marker,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DiffLine {
    pub(crate) kind: DiffLineKind,
    /// The line without its diff prefix.
    pub(crate) text: String,
    pub(crate) old_line: Option<u64>,
    pub(crate) new_line: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DiffHunk {
    /// The `@@ -a,b +c,d @@` line, including any trailing section heading.
    pub(crate) heading: String,
    pub(crate) lines: Vec<DiffLine>,
    pub(crate) additions: usize,
    pub(crate) deletions: usize,
}

/// One row of a side-by-side view, as indices into the hunk's lines.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SplitRow {
    /// A `\ No newline at end of file` note, which belongs to neither side.
    Note { line: usize },
    /// A line against its counterpart. Either side is empty when its half of
    /// the block is shorter, which is the blank cell a diff editor shows.
    Pair {
        left: Option<usize>,
        right: Option<usize>,
    },
}

impl DiffHunk {
    /// Lines paired left against right the way a side-by-side editor shows
    /// them: a run of removals lines up with the run of additions that replaced
    /// it, and a line of context sits on both sides.
    pub(crate) fn split_rows(&self) -> Vec<SplitRow> {
        let kind = |index: usize| self.lines[index].kind;
        let mut rows = Vec::new();
        let mut index = 0;
        while index < self.lines.len() {
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
                    while index < self.lines.len() && kind(index) == DiffLineKind::Removed {
                        index += 1;
                    }
                    let removed_len = index - removed;
                    let added = index;
                    while index < self.lines.len() && kind(index) == DiffLineKind::Added {
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

/// One file's unified diff, split so each hunk can be applied on its own.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FileDiff {
    /// Everything above the first hunk: the header a patch has to carry.
    header: String,
    pub(crate) hunks: Vec<DiffHunk>,
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
            // Everything above the first hunk is header; anything below the
            // last one belongs to another file's block.
            if !hunks.is_empty() {
                break;
            }
            header.push_str(line);
            header.push('\n');
            index += 1;
        }
        Self { header, hunks }
    }

    /// A patch for one hunk, ready to be applied on its own.
    pub(crate) fn patch_for(&self, index: usize) -> Option<String> {
        let hunk = self.hunks.get(index)?;
        let mut patch = String::with_capacity(self.header.len() + hunk.heading.len() + 64);
        patch.push_str(&self.header);
        patch.push_str(&hunk.heading);
        patch.push('\n');
        for line in &hunk.lines {
            match line.kind {
                DiffLineKind::Marker => patch.push_str("\\ No newline at end of file\n"),
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

/// The starting line numbers of `@@ -old,count +new,count @@`.
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
