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
    // One line in, three out: the second and third rows are additions with
    // nothing to pair with, which is the blank cell on the left.
    let diff = FileDiff::parse(
        "diff --git a/f b/f\n--- a/f\n+++ b/f\n@@ -1,2 +1,4 @@\n keep\n-old\n+new\n+much\n+more\n",
    );
    let rows = diff.hunks[0].split_rows();
    assert_eq!(rows.len(), 4);
    // Context sits on both sides.
    assert_eq!(
        rows[0],
        SplitRow::Pair {
            left: Some(0),
            right: Some(0)
        }
    );
    // The removal and the first addition share a row.
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
    let rows = diff.hunks[0].split_rows();
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
    let rows = diff.hunks[0].split_rows();
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
fn hunk_actions_name_their_target() {
    assert_eq!(HunkApply::Stage.label(), "Stage hunk");
    assert_eq!(HunkApply::Unstage.label(), "Unstage hunk");
    assert_eq!(HunkApply::Revert.label(), "Revert hunk");
}
