use super::*;

const TWO_HUNKS: &str = "\
diff --git a/src/app.rs b/src/app.rs
index 1111111..2222222 100644
--- a/src/app.rs
+++ b/src/app.rs
@@ -1,4 +1,5 @@ fn main() {
 fn main() {
-    old();
+    new();
+    extra();
     keep();
 }
@@ -10,3 +11,3 @@ fn other() {
 fn other() {
-    before();
+    after();
 }
";

#[test]
fn parse_keeps_the_header_and_every_hunk() {
    let diff = FileDiff::parse(TWO_HUNKS);
    assert_eq!(diff.hunks.len(), 2);
    assert!(diff.header.contains("diff --git a/src/app.rs b/src/app.rs"));
    assert!(diff.header.contains("--- a/src/app.rs"));
    assert_eq!(diff.hunks[0].heading, "@@ -1,4 +1,5 @@ fn main() {");
    assert_eq!(diff.hunks[1].heading, "@@ -10,3 +11,3 @@ fn other() {");
    assert_eq!(diff.hunks[0].additions, 2);
    assert_eq!(diff.hunks[0].deletions, 1);
    assert_eq!(diff.hunks[1].additions, 1);
    assert_eq!(diff.hunks[1].deletions, 1);
}

#[test]
fn parse_numbers_lines_from_the_heading() {
    let diff = FileDiff::parse(TWO_HUNKS);
    let first = &diff.hunks[0].lines;
    assert_eq!(
        first
            .iter()
            .map(|line| (line.kind, line.old_line, line.new_line))
            .collect::<Vec<_>>(),
        vec![
            (DiffLineKind::Context, Some(1), Some(1)),
            (DiffLineKind::Removed, Some(2), None),
            (DiffLineKind::Added, None, Some(2)),
            (DiffLineKind::Added, None, Some(3)),
            (DiffLineKind::Context, Some(3), Some(4)),
            (DiffLineKind::Context, Some(4), Some(5)),
        ]
    );
    assert_eq!(first[1].text, "    old();");
    assert!(
        diff.hunks[1]
            .lines
            .iter()
            .any(|line| line.kind == DiffLineKind::Removed && line.old_line == Some(11))
    );
}

#[test]
fn a_single_hunk_patch_carries_the_header_and_only_that_hunk() {
    let diff = FileDiff::parse(TWO_HUNKS);
    let patch = diff.patch_for(1).expect("second hunk");
    assert!(patch.starts_with("diff --git a/src/app.rs b/src/app.rs\n"));
    assert!(patch.contains("@@ -10,3 +11,3 @@ fn other() {"));
    assert!(patch.contains("-    before();\n+    after();\n"));
    assert!(!patch.contains("extra()"));
    assert!(!patch.contains("@@ -1,4 +1,5 @@"));
    assert!(diff.patch_for(2).is_none());
}

#[test]
fn parse_keeps_a_missing_newline_marker_with_its_hunk() {
    let diff = FileDiff::parse(
        "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-old\n\\ No newline at end of file\n+new\n\\ No newline at end of file\n",
    );
    assert_eq!(diff.hunks.len(), 1);
    let kinds = diff.hunks[0]
        .lines
        .iter()
        .map(|line| line.kind)
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        vec![
            DiffLineKind::Removed,
            DiffLineKind::Marker,
            DiffLineKind::Added,
            DiffLineKind::Marker,
        ]
    );
    let patch = diff.patch_for(0).expect("hunk");
    assert_eq!(patch.matches("\\ No newline at end of file").count(), 2);
}

#[test]
fn parse_handles_new_deleted_and_binary_files() {
    let added = FileDiff::parse(
        "diff --git a/fresh b/fresh\nnew file mode 100644\nindex 0000000..e69de29\n--- /dev/null\n+++ b/fresh\n@@ -0,0 +1,2 @@\n+one\n+two\n",
    );
    assert_eq!(added.hunks.len(), 1);
    assert_eq!(added.hunks[0].additions, 2);
    assert!(
        added.hunks[0]
            .lines
            .iter()
            .all(|line| line.old_line.is_none())
    );
    assert!(
        added
            .patch_for(0)
            .expect("hunk")
            .contains("new file mode 100644")
    );

    let deleted = FileDiff::parse(
        "diff --git a/gone b/gone\ndeleted file mode 100644\n--- a/gone\n+++ /dev/null\n@@ -1,2 +0,0 @@\n-one\n-two\n",
    );
    assert_eq!(deleted.hunks.len(), 1);
    assert_eq!(deleted.hunks[0].deletions, 2);
    assert!(
        deleted.hunks[0]
            .lines
            .iter()
            .all(|line| line.new_line.is_none())
    );

    let binary = FileDiff::parse(
        "diff --git a/logo.png b/logo.png\nBinary files a/logo.png and b/logo.png differ\n",
    );
    assert!(binary.hunks.is_empty());
    assert!(binary.patch_for(0).is_none());
}

#[test]
fn parse_ignores_an_empty_patch() {
    assert_eq!(FileDiff::parse("").hunks, Vec::new());
}

#[test]
fn split_rows_line_a_replacement_up_left_against_right() {
    let diff = FileDiff::parse(
        "diff --git a/f b/f\n--- a/f\n+++ b/f\n@@ -1,2 +1,4 @@\n keep\n-old\n+new\n+much\n+more\n",
    );
    let hunk_lines = diff.hunks[0].lines.len();
    let rows = diff.hunks[0].pair_segment(0..hunk_lines);
    assert_eq!(rows.len(), 4);
    assert_eq!(
        rows[0],
        SplitRow::Pair {
            left: Some(0),
            right: Some(0)
        }
    );
    assert_eq!(
        rows[1],
        SplitRow::Pair {
            left: Some(1),
            right: Some(2)
        }
    );
    assert_eq!(
        rows[2],
        SplitRow::Pair {
            left: None,
            right: Some(3)
        }
    );
    assert_eq!(
        rows[3],
        SplitRow::Pair {
            left: None,
            right: Some(4)
        }
    );
}

#[test]
fn split_rows_pair_a_shorter_replacement_with_a_blank_right() {
    let diff = FileDiff::parse(
        "diff --git a/f b/f\n--- a/f\n+++ b/f\n@@ -1,3 +1,2 @@\n-one\n-two\n-three\n+replacement\n",
    );
    let hunk_lines = diff.hunks[0].lines.len();
    let rows = diff.hunks[0].pair_segment(0..hunk_lines);
    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows[0],
        SplitRow::Pair {
            left: Some(0),
            right: Some(3)
        }
    );
    assert_eq!(
        rows[1],
        SplitRow::Pair {
            left: Some(1),
            right: None
        }
    );
    assert_eq!(
        rows[2],
        SplitRow::Pair {
            left: Some(2),
            right: None
        }
    );
}

#[test]
fn split_rows_keep_a_no_newline_note_on_its_own() {
    let diff = FileDiff::parse(
        "diff --git a/f b/f\n--- a/f\n+++ b/f\n@@ -1 +1 @@\n-old\n\\ No newline at end of file\n+new\n\\ No newline at end of file\n",
    );
    let hunk_lines = diff.hunks[0].lines.len();
    let rows = diff.hunks[0].pair_segment(0..hunk_lines);
    assert_eq!(
        rows[0],
        SplitRow::Pair {
            left: Some(0),
            right: None
        }
    );
    assert_eq!(rows[1], SplitRow::Note { line: 1 });
    assert_eq!(
        rows[2],
        SplitRow::Pair {
            left: None,
            right: Some(2)
        }
    );
    assert_eq!(rows[3], SplitRow::Note { line: 3 });
}

#[test]
fn rows_draw_the_whole_file_in_order() {
    let mut diff = FileDiff::parse(TWO_HUNKS);
    diff.absorb_unchanged(&FileDiff::parse(TWO_HUNKS_FULL));
    let rows = diff.rows(false, false, &[]);
    assert_eq!(rows.len(), 17);
    let block = rows
        .iter()
        .position(|row| matches!(row, DiffRow::Block { hunk: 0 }))
        .expect("the first block");
    assert_eq!(
        rows[block - 1],
        DiffRow::Line {
            source: DiffSource::Unchanged(0),
            line: 0
        }
    );
    assert_eq!(
        rows[block + 1],
        DiffRow::Line {
            source: DiffSource::Hunk(0),
            line: 1
        }
    );
    assert_eq!(
        rows.last(),
        Some(&DiffRow::Line {
            source: DiffSource::Unchanged(2),
            line: 0
        })
    );
}

#[test]
fn rows_pair_a_block_side_by_side() {
    let mut diff = FileDiff::parse(TWO_HUNKS);
    diff.absorb_unchanged(&FileDiff::parse(TWO_HUNKS_FULL));
    let rows = diff.rows(true, false, &[]);
    assert!(rows.contains(&DiffRow::Split {
        source: DiffSource::Unchanged(1),
        row: SplitRow::Pair {
            left: Some(0),
            right: Some(0)
        }
    }));
    assert!(rows.contains(&DiffRow::Split {
        source: DiffSource::Hunk(0),
        row: SplitRow::Pair {
            left: Some(1),
            right: Some(2)
        }
    }));
    assert!(rows.contains(&DiffRow::Split {
        source: DiffSource::Hunk(0),
        row: SplitRow::Pair {
            left: None,
            right: Some(3)
        }
    }));
}

#[test]
fn rows_keep_a_no_newline_note_in_the_paired_reading() {
    let diff = FileDiff::parse(
        "diff --git a/f b/f\n--- a/f\n+++ b/f\n@@ -1 +1 @@\n-old\n\\ No newline at end of file\n+new\n\\ No newline at end of file\n",
    );
    let split = diff.rows(true, false, &[]);
    assert!(split.contains(&DiffRow::Split {
        source: DiffSource::Hunk(0),
        row: SplitRow::Note { line: 1 }
    }));
    assert!(split.contains(&DiffRow::Split {
        source: DiffSource::Hunk(0),
        row: SplitRow::Note { line: 3 }
    }));
}

#[test]
fn rows_of_a_file_without_changes_are_empty() {
    assert!(FileDiff::parse("").rows(false, false, &[]).is_empty());
    assert!(FileDiff::parse("").rows(true, false, &[]).is_empty());
    assert!(
        FileDiff::parse("diff --git a/logo.png b/logo.png\nBinary files differ\n")
            .rows(true, false, &[])
            .is_empty()
    );
}

const TWO_HUNKS_FULL: &str = "\
diff --git a/src/app.rs b/src/app.rs
index 1111111..2222222 100644
--- a/src/app.rs
+++ b/src/app.rs
@@ -1,12 +1,13 @@ fn main() {
 fn main() {
-    old();
+    new();
+    extra();
     keep();
 }
 // unchanged one
 // unchanged two
 // unchanged three
 // unchanged four
 // unchanged five
 fn other() {
-    before();
+    after();
 }
";

#[test]
fn a_run_of_unchanged_lines_takes_the_context_with_it() {
    let mut diff = FileDiff::parse(TWO_HUNKS);
    assert_eq!(
        diff.spans().iter().map(Vec::len).collect::<Vec<_>>(),
        vec![1, 3, 1]
    );
    diff.absorb_unchanged(&FileDiff::parse(TWO_HUNKS_FULL));
    assert_eq!(
        diff.spans().iter().map(Vec::len).collect::<Vec<_>>(),
        vec![1, 8, 1]
    );
    assert_eq!(
        diff.spans()[1]
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>(),
        vec![
            "    keep();",
            "}",
            "// unchanged one",
            "// unchanged two",
            "// unchanged three",
            "// unchanged four",
            "// unchanged five",
            "fn other() {"
        ]
    );
    assert!(
        diff.spans()
            .iter()
            .flatten()
            .all(|line| line.kind == crate::repository::DiffLineKind::Context)
    );
}

#[test]
fn folding_folds_a_whole_run_of_unchanged_lines() {
    let mut diff = FileDiff::parse(TWO_HUNKS);
    diff.absorb_unchanged(&FileDiff::parse(TWO_HUNKS_FULL));
    let folded = diff.rows(false, true, &[]);
    assert_eq!(
        folded
            .iter()
            .filter_map(|row| match row {
                DiffRow::Band {
                    span,
                    lines,
                    folded,
                } => Some((*span, *lines, *folded)),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![(0, 1, true), (1, 8, true), (2, 1, true)]
    );
    assert_eq!(
        folded
            .iter()
            .filter(|row| matches!(row, DiffRow::Block { .. }))
            .count(),
        2
    );
    assert_eq!(folded.len(), 2 + 3 + 2 + 3);

    let opened = diff.rows(false, true, &[1]);
    let band = opened
        .iter()
        .position(|row| matches!(row, DiffRow::Band { span: 1, .. }))
        .expect("the band of the opened run");
    assert!(matches!(opened[band], DiffRow::Band { folded: false, .. }));
    assert_eq!(
        opened[band + 1],
        DiffRow::Line {
            source: DiffSource::Unchanged(1),
            line: 0
        }
    );
    assert_eq!(
        opened[band + 8],
        DiffRow::Line {
            source: DiffSource::Unchanged(1),
            line: 7
        }
    );
}

#[test]
fn a_new_file_is_read_as_one_side() {
    let added = FileDiff::parse(
        "diff --git a/fresh b/fresh\nnew file mode 100644\nindex 0000000..e69de29\n--- /dev/null\n+++ b/fresh\n@@ -0,0 +1,2 @@\n+one\n+two\n",
    );
    assert!(added.is_new_file());
    assert_eq!(
        added.rows(true, false, &[]),
        vec![
            DiffRow::Block { hunk: 0 },
            DiffRow::Line {
                source: DiffSource::Hunk(0),
                line: 0
            },
            DiffRow::Line {
                source: DiffSource::Hunk(0),
                line: 1
            },
        ]
    );
    assert!(!FileDiff::parse(TWO_HUNKS).is_new_file());
}

fn cells(text: &str) -> f32 {
    text.chars()
        .map(|character| if character.is_ascii() { 1.0 } else { 2.0 })
        .sum()
}

#[test]
fn the_widest_line_of_each_side_measures_the_whole_file() {
    let mut diff = FileDiff::parse(
        "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1,1 +1,1 @@\n-old\n+new\n@@ -6,1 +6,1 @@\n-late\n+later\n",
    );
    assert_eq!(diff.widest_sides(cells), SideWidths { old: 4.0, new: 5.0 });
    let full = FileDiff::parse(
        "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1,6 +1,6 @@\n-old\n+new\n a line\n a much longer unchanged line\n last\n-late\n+later\n",
    );
    diff.absorb_unchanged(&full);
    assert_eq!(
        diff.widest_sides(cells),
        SideWidths {
            old: 28.0,
            new: 28.0
        }
    );
    let empty = FileDiff::parse("").widest_sides(cells);
    assert_eq!(empty.old.max(empty.new), 0.0);
}

#[test]
fn the_widest_line_is_the_one_that_needs_the_most_room() {
    let diff = FileDiff::parse(
        "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1,3 +1,3 @@\n a line of twenty-one\n-日本語のテキストでした\n+english text\n",
    );
    let widths = diff.widest_sides(cells);
    assert_eq!(
        widths,
        SideWidths {
            old: 22.0,
            new: 20.0
        }
    );
    assert_eq!(widths.old.max(widths.new), 22.0);
}

#[test]
fn the_note_about_a_missing_newline_is_measured_with_its_line() {
    let diff = FileDiff::parse(
        "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-old\n\\ No newline at end of file\n+new\n\\ No newline at end of file\n",
    );
    let widths = diff.widest_sides(cells);
    assert_eq!(
        widths.old.max(widths.new),
        NO_NEWLINE_NOTE.chars().count() as f32
    );
}

#[test]
fn hunk_actions_name_their_target() {
    assert_eq!(HunkApply::Stage.label(), "Stage hunk");
    assert_eq!(HunkApply::Unstage.label(), "Unstage hunk");
    assert_eq!(HunkApply::Revert.label(), "Revert hunk");
}
