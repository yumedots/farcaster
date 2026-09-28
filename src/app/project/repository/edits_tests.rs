use super::*;

#[test]
fn checkboxes_select_directly_and_deselect_the_last_path() {
    let mut selection = FileSelection::default();
    selection.toggle("a".into());
    assert_eq!(selection.paths, BTreeSet::from([PathBuf::from("a")]));
    selection.toggle("b".into());
    selection.toggle("a".into());
    assert_eq!(selection.paths, BTreeSet::from([PathBuf::from("b")]));
    selection.toggle("b".into());
    assert!(selection.paths.is_empty());
}

#[test]
fn modifier_clicks_toggle_and_shift_clicks_extend_from_the_anchor() {
    let visible = ["a", "b", "c", "d"].map(PathBuf::from);
    let mut selection = FileSelection::default();
    selection.toggle_from(visible[1].clone());
    assert_eq!(selection.paths, BTreeSet::from([visible[1].clone()]));

    // The anchor stays on the last modifier click, so shift extends from it.
    selection.extend_to(visible[3].clone(), &visible);
    assert_eq!(
        selection.paths,
        BTreeSet::from([visible[1].clone(), visible[2].clone(), visible[3].clone()])
    );

    // A downward range adds to the existing selection instead of replacing it.
    selection.extend_to(visible[0].clone(), &visible);
    assert_eq!(selection.paths.len(), 4);

    // Shift without an anchor behaves like a modifier click.
    let mut fresh = FileSelection::default();
    fresh.extend_to(visible[2].clone(), &visible);
    assert_eq!(fresh.paths, BTreeSet::from([visible[2].clone()]));

    // A row can be toggled off while it stays the anchor for the next range.
    selection.toggle_from(visible[1].clone());
    assert!(!selection.paths.contains(&visible[1]));
    selection.extend_to(visible[3].clone(), &visible);
    assert!(selection.paths.contains(&visible[1]));
}
