use super::*;

#[test]
fn change_rows_keep_staged_and_working_tree_layers_apart() {
    assert_ne!(
        crate::repository::ChangeLayer::Index,
        crate::repository::ChangeLayer::WorkingTree
    );
    assert_eq!(group_title(crate::repository::ChangeLayer::Index), "Staged");
    assert_eq!(
        group_title(crate::repository::ChangeLayer::WorkingTree),
        "Working tree"
    );
}
