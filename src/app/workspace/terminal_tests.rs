use gpui::EntityId;

use super::{TerminalLayout, TerminalPane, TerminalSplitDirection};

fn pane(id: u64) -> EntityId {
    EntityId::from(id)
}

fn layout(primary: u64) -> TerminalLayout {
    TerminalLayout {
        root: TerminalPane::Leaf(pane(primary)),
        focused: pane(primary),
        panes: std::collections::HashMap::new(),
    }
}

#[test]
fn splitting_the_focused_pane_places_the_new_pane_beside_it() {
    let mut layout = layout(1);
    layout.insert_id(TerminalSplitDirection::Right, pane(2));
    assert!(matches!(
        &layout.root,
        TerminalPane::Split {
            direction: TerminalSplitDirection::Right,
            first,
            second,
            ..
        } if matches!(**first, TerminalPane::Leaf(id) if id == pane(1))
            && matches!(**second, TerminalPane::Leaf(id) if id == pane(2))
    ));
    assert_eq!(layout.focused_id(), pane(2));
    assert_eq!(layout.leaf_count(), 2);
}

#[test]
fn splits_nest_from_the_focused_pane() {
    let mut layout = layout(1);
    layout.insert_id(TerminalSplitDirection::Right, pane(2));
    layout.insert_id(TerminalSplitDirection::Down, pane(3));
    assert_eq!(layout.focused_id(), pane(3));
    assert_eq!(layout.leaf_count(), 3);
    let TerminalPane::Split {
        direction, second, ..
    } = &layout.root
    else {
        panic!("expected a split root");
    };
    assert_eq!(*direction, TerminalSplitDirection::Right);
    let TerminalPane::Split { direction, .. } = second.as_ref() else {
        panic!("expected the focused pane to split again");
    };
    assert_eq!(*direction, TerminalSplitDirection::Down);
}

#[test]
fn removing_the_focused_pane_focuses_its_neighbor() {
    let mut layout = layout(1);
    layout.insert_id(TerminalSplitDirection::Right, pane(2));
    layout.insert_id(TerminalSplitDirection::Right, pane(3));
    assert!(layout.remove(pane(3)));
    assert_eq!(layout.focused_id(), pane(2));
    assert_eq!(layout.leaf_count(), 2);
    assert!(layout.contains(pane(1)));
    assert!(layout.contains(pane(2)));
    assert!(!layout.contains(pane(3)));
}

#[test]
fn removing_a_pane_collapses_the_split() {
    let mut layout = layout(1);
    layout.insert_id(TerminalSplitDirection::Right, pane(2));
    assert!(layout.remove(pane(2)));
    assert_eq!(layout.single_leaf_id(), Some(pane(1)));
    assert_eq!(layout.focused_id(), pane(1));
}

#[test]
fn the_last_pane_cannot_be_removed() {
    let mut layout = layout(1);
    assert!(!layout.remove(pane(1)));
    assert_eq!(layout.leaf_count(), 1);
    layout.insert_id(TerminalSplitDirection::Down, pane(2));
    assert!(layout.remove(pane(2)));
    assert!(!layout.remove(pane(1)));
    assert_eq!(layout.single_leaf_id(), Some(pane(1)));
}

#[test]
fn focus_only_moves_to_panes_in_the_layout() {
    let mut layout = layout(1);
    layout.insert_id(TerminalSplitDirection::Right, pane(2));
    layout.set_focused(pane(9));
    assert_eq!(layout.focused_id(), pane(2));
    layout.set_focused(pane(1));
    assert_eq!(layout.focused_id(), pane(1));
}

#[test]
fn ratios_update_along_the_split_path() {
    let mut layout = layout(1);
    layout.insert_id(TerminalSplitDirection::Right, pane(2));
    layout.insert_id(TerminalSplitDirection::Down, pane(3));
    assert_eq!(layout.ratio(&[]), Some(0.5));
    assert_eq!(layout.ratio(&[true]), Some(0.5));
    assert!(layout.set_ratio(&[], 0.3));
    assert!(layout.set_ratio(&[true], 0.7));
    assert_eq!(layout.ratio(&[]), Some(0.3));
    assert_eq!(layout.ratio(&[true]), Some(0.7));
    assert!(!layout.set_ratio(&[false, false], 0.5));
    assert_eq!(layout.ratio(&[false]), None);
}

#[test]
fn leaf_ids_follow_render_order() {
    let mut layout = layout(1);
    layout.insert_id(TerminalSplitDirection::Right, pane(2));
    layout.insert_id(TerminalSplitDirection::Down, pane(3));
    assert_eq!(layout.leaf_ids(), vec![pane(1), pane(2), pane(3)]);
}
