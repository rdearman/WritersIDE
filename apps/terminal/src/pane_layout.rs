//! Recursive geometry model for terminal panes.
//!
//! The tree stores layout only; documents and editor buffers remain owned by
//! the application state. This lets a pane be split without copying prose.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Orientation {
    Vertical,
    Horizontal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PaneRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PaneNode {
    Leaf(u64),
    Split {
        orientation: Orientation,
        /// Basis points assigned to the first child (1..=9999).
        ratio: u16,
        first: Box<PaneNode>,
        second: Box<PaneNode>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PaneLayout {
    root: PaneNode,
    focused: u64,
    next_id: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutError {
    UnknownPane,
    InsufficientSpace,
    CannotCloseOnlyPane,
    NoParentSplit,
    PaneIdExhausted,
}

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::UnknownPane => "the selected window no longer exists",
            Self::InsufficientSpace => "the terminal area is too small for another split",
            Self::CannotCloseOnlyPane => "there is only one window open",
            Self::NoParentSplit => "the focused window has no divider to resize",
            Self::PaneIdExhausted => "no more window identifiers are available",
        };
        f.write_str(message)
    }
}

impl std::error::Error for LayoutError {}

impl Default for PaneLayout {
    fn default() -> Self {
        Self {
            root: PaneNode::Leaf(1),
            focused: 1,
            next_id: 2,
        }
    }
}

impl PaneLayout {
    pub fn root(&self) -> &PaneNode {
        &self.root
    }

    pub fn focused(&self) -> u64 {
        self.focused
    }

    pub fn pane_count(&self) -> usize {
        leaves(&self.root).len()
    }

    pub fn panes(&self) -> Vec<u64> {
        leaves(&self.root)
    }

    pub fn focus(&mut self, id: u64) -> Result<(), LayoutError> {
        if !contains(&self.root, id) {
            return Err(LayoutError::UnknownPane);
        }
        self.focused = id;
        Ok(())
    }

    pub fn focus_next(&mut self, reverse: bool) {
        let panes = self.panes();
        if panes.len() < 2 {
            return;
        }
        let index = panes.iter().position(|id| *id == self.focused).unwrap_or(0);
        let next = if reverse {
            (index + panes.len() - 1) % panes.len()
        } else {
            (index + 1) % panes.len()
        };
        self.focused = panes[next];
    }

    /// Split the focused leaf, retaining its identity as the first child.
    /// The new second child receives focus. Existing sibling geometry is
    /// unchanged because only this leaf is replaced.
    pub fn split(
        &mut self,
        orientation: Orientation,
        ratio: u16,
        bounds: PaneRect,
        min_width: u16,
        min_height: u16,
    ) -> Result<u64, LayoutError> {
        if !can_split(bounds, orientation, ratio, min_width, min_height) {
            return Err(LayoutError::InsufficientSpace);
        }
        let new_id = self.next_id;
        let next_id = new_id.checked_add(1).ok_or(LayoutError::PaneIdExhausted)?;
        let replacement = PaneNode::Split {
            orientation,
            ratio: ratio.clamp(1, 9_999),
            first: Box::new(PaneNode::Leaf(self.focused)),
            second: Box::new(PaneNode::Leaf(new_id)),
        };
        if !replace_leaf(&mut self.root, self.focused, replacement) {
            return Err(LayoutError::UnknownPane);
        }
        self.next_id = next_id;
        self.focused = new_id;
        Ok(new_id)
    }

    /// Adds a pane with an existing stable ID beside the focused leaf. This is
    /// used to restore a document view that was closed while its document
    /// remained open in the application state.
    pub fn split_with_existing_id(
        &mut self,
        orientation: Orientation,
        ratio: u16,
        bounds: PaneRect,
        min_width: u16,
        min_height: u16,
        id: u64,
    ) -> Result<(), LayoutError> {
        if contains(&self.root, id) {
            return Err(LayoutError::UnknownPane);
        }
        let new_id = self.split(orientation, ratio, bounds, min_width, min_height)?;
        if !replace_leaf(&mut self.root, new_id, PaneNode::Leaf(id)) {
            return Err(LayoutError::UnknownPane);
        }
        self.focused = id;
        Ok(())
    }

    /// Remove one leaf and promote its sibling subtree into the same region.
    pub fn close(&mut self, id: u64) -> Result<(), LayoutError> {
        if self.pane_count() == 1 && self.focused == id {
            return Err(LayoutError::CannotCloseOnlyPane);
        }
        if !contains(&self.root, id) {
            return Err(LayoutError::UnknownPane);
        }
        self.root = remove_leaf(self.root.clone(), id).ok_or(LayoutError::UnknownPane)?;
        if self.focused == id {
            self.focused = first_leaf(&self.root).ok_or(LayoutError::UnknownPane)?;
        }
        Ok(())
    }

    /// Keep the focused leaf and discard all other layout nodes.
    pub fn only_focused(&mut self) {
        self.root = PaneNode::Leaf(self.focused);
    }

    /// Set the divider ratio on the split immediately containing `id`.
    pub fn resize_parent_split(&mut self, id: u64, ratio: u16) -> Result<(), LayoutError> {
        if resize_parent(&mut self.root, id, ratio.clamp(1, 9_999)) {
            Ok(())
        } else {
            Err(LayoutError::UnknownPane)
        }
    }

    /// Adjusts the nearest divider so the focused pane grows or shrinks by a
    /// small deterministic step. The ratio is stored in basis points.
    pub fn resize_focused_parent(&mut self, grow: bool) -> Result<(), LayoutError> {
        if resize_focused_parent(&mut self.root, self.focused, grow) {
            Ok(())
        } else if contains(&self.root, self.focused) {
            Err(LayoutError::NoParentSplit)
        } else {
            Err(LayoutError::UnknownPane)
        }
    }

    pub fn rectangles(
        &self,
        bounds: PaneRect,
        min_width: u16,
        min_height: u16,
    ) -> Vec<(u64, PaneRect)> {
        let mut output = Vec::new();
        layout_node(&self.root, bounds, min_width, min_height, &mut output);
        output
    }

    pub fn is_valid_with_panes(&self, available: &[u64]) -> bool {
        let mut actual = self.panes();
        actual.sort_unstable();
        let mut expected = available.to_vec();
        expected.sort_unstable();
        actual == expected
            && actual.contains(&self.focused)
            && self.next_id > actual.iter().copied().max().unwrap_or(0)
    }

    /// Validates a temporarily maximized layout that contains a subset of the
    /// application's open panes. Every referenced pane must still exist.
    pub fn is_valid_subset_of_panes(&self, available: &[u64]) -> bool {
        let actual = self.panes();
        let unique = actual
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        unique.len() == actual.len()
            && valid_node(&self.root)
            && actual.iter().all(|id| available.contains(id))
            && actual.contains(&self.focused)
            && self.next_id > actual.iter().copied().max().unwrap_or(0)
    }

    /// Reassigns pane identities while retaining the recursive split tree.
    /// Every existing leaf must have exactly one mapping, and destination IDs
    /// must be unique. This lets saved layouts reconnect to currently open
    /// views by their stable source keys rather than stale runtime pane IDs.
    pub fn remap_panes(&mut self, mapping: &[(u64, u64)]) -> Result<(), LayoutError> {
        let existing = self.panes();
        let mut destinations = std::collections::BTreeSet::new();
        if mapping.len() != existing.len()
            || existing
                .iter()
                .any(|id| !mapping.iter().any(|(from, _)| from == id))
            || mapping.iter().any(|(from, _)| !existing.contains(from))
            || mapping.iter().any(|(_, to)| !destinations.insert(*to))
        {
            return Err(LayoutError::UnknownPane);
        }
        let remap = |id: &mut u64| {
            if let Some((_, destination)) = mapping.iter().find(|(source, _)| source == id) {
                *id = *destination;
            }
        };
        fn visit(node: &mut PaneNode, remap: &impl Fn(&mut u64)) {
            match node {
                PaneNode::Leaf(id) => remap(id),
                PaneNode::Split { first, second, .. } => {
                    visit(first, remap);
                    visit(second, remap);
                }
            }
        }
        visit(&mut self.root, &remap);
        remap(&mut self.focused);
        self.next_id = destinations
            .iter()
            .copied()
            .max()
            .and_then(|largest| largest.checked_add(1))
            .ok_or(LayoutError::PaneIdExhausted)?;
        Ok(())
    }
}

fn valid_node(node: &PaneNode) -> bool {
    match node {
        PaneNode::Leaf(_) => true,
        PaneNode::Split {
            ratio,
            first,
            second,
            ..
        } => (1..=9_999).contains(ratio) && valid_node(first) && valid_node(second),
    }
}

pub fn can_split(
    bounds: PaneRect,
    orientation: Orientation,
    ratio: u16,
    min_width: u16,
    min_height: u16,
) -> bool {
    let ratio = ratio.clamp(1, 9_999) as u32;
    let extent = match orientation {
        Orientation::Vertical => bounds.width,
        Orientation::Horizontal => bounds.height,
    } as u32;
    let first = extent.saturating_mul(ratio) / 10_000;
    let second = extent.saturating_sub(first);
    match orientation {
        Orientation::Vertical => first >= min_width as u32 && second >= min_width as u32,
        Orientation::Horizontal => first >= min_height as u32 && second >= min_height as u32,
    }
}

fn layout_node(
    node: &PaneNode,
    bounds: PaneRect,
    min_width: u16,
    min_height: u16,
    output: &mut Vec<(u64, PaneRect)>,
) {
    match node {
        PaneNode::Leaf(id) => output.push((*id, bounds)),
        PaneNode::Split {
            orientation,
            ratio,
            first,
            second,
        } => {
            let extent = match orientation {
                Orientation::Vertical => bounds.width,
                Orientation::Horizontal => bounds.height,
            } as u32;
            let mut first_extent = (extent * (*ratio as u32) / 10_000) as u16;
            let min = match orientation {
                Orientation::Vertical => min_width,
                Orientation::Horizontal => min_height,
            };
            if extent >= u32::from(min) * 2 {
                first_extent = first_extent.clamp(min, (extent as u16).saturating_sub(min));
            } else {
                first_extent = (extent / 2) as u16;
            }
            let second_extent = (extent as u16).saturating_sub(first_extent);
            let (first_rect, second_rect) = match orientation {
                Orientation::Vertical => (
                    PaneRect {
                        width: first_extent,
                        ..bounds
                    },
                    PaneRect {
                        x: bounds.x.saturating_add(first_extent),
                        width: second_extent,
                        ..bounds
                    },
                ),
                Orientation::Horizontal => (
                    PaneRect {
                        height: first_extent,
                        ..bounds
                    },
                    PaneRect {
                        y: bounds.y.saturating_add(first_extent),
                        height: second_extent,
                        ..bounds
                    },
                ),
            };
            layout_node(first, first_rect, min_width, min_height, output);
            layout_node(second, second_rect, min_width, min_height, output);
        }
    }
}

fn leaves(node: &PaneNode) -> Vec<u64> {
    match node {
        PaneNode::Leaf(id) => vec![*id],
        PaneNode::Split { first, second, .. } => {
            let mut result = leaves(first);
            result.extend(leaves(second));
            result
        }
    }
}

fn first_leaf(node: &PaneNode) -> Option<u64> {
    match node {
        PaneNode::Leaf(id) => Some(*id),
        PaneNode::Split { first, .. } => first_leaf(first),
    }
}

fn contains(node: &PaneNode, id: u64) -> bool {
    match node {
        PaneNode::Leaf(candidate) => *candidate == id,
        PaneNode::Split { first, second, .. } => contains(first, id) || contains(second, id),
    }
}

fn replace_leaf(node: &mut PaneNode, id: u64, replacement: PaneNode) -> bool {
    match node {
        PaneNode::Leaf(candidate) if *candidate == id => {
            *node = replacement;
            true
        }
        PaneNode::Leaf(_) => false,
        PaneNode::Split { first, second, .. } => {
            replace_leaf(first, id, replacement.clone()) || replace_leaf(second, id, replacement)
        }
    }
}

fn resize_parent(node: &mut PaneNode, id: u64, ratio: u16) -> bool {
    match node {
        PaneNode::Leaf(_) => false,
        PaneNode::Split {
            ratio: current,
            first,
            second,
            ..
        } => {
            if contains(first, id) || contains(second, id) {
                if matches!(first.as_ref(), PaneNode::Leaf(child) if *child == id)
                    || matches!(second.as_ref(), PaneNode::Leaf(child) if *child == id)
                {
                    *current = ratio;
                    true
                } else {
                    resize_parent(first, id, ratio) || resize_parent(second, id, ratio)
                }
            } else {
                false
            }
        }
    }
}

fn resize_focused_parent(node: &mut PaneNode, id: u64, grow: bool) -> bool {
    let PaneNode::Split {
        ratio,
        first,
        second,
        ..
    } = node
    else {
        return false;
    };
    if matches!(first.as_ref(), PaneNode::Leaf(child) if *child == id) {
        *ratio = if grow {
            ratio.saturating_add(500).min(9_000)
        } else {
            ratio.saturating_sub(500).max(1_000)
        };
        return true;
    }
    if matches!(second.as_ref(), PaneNode::Leaf(child) if *child == id) {
        *ratio = if grow {
            ratio.saturating_sub(500).max(1_000)
        } else {
            ratio.saturating_add(500).min(9_000)
        };
        return true;
    }
    if contains(first, id) {
        resize_focused_parent(first, id, grow)
    } else {
        resize_focused_parent(second, id, grow)
    }
}

fn remove_leaf(node: PaneNode, id: u64) -> Option<PaneNode> {
    match node {
        PaneNode::Leaf(candidate) if candidate == id => None,
        leaf @ PaneNode::Leaf(_) => Some(leaf),
        PaneNode::Split {
            orientation,
            ratio,
            first,
            second,
        } => {
            let first = remove_leaf(*first, id);
            let second = remove_leaf(*second, id);
            match (first, second) {
                (None, Some(sibling)) | (Some(sibling), None) => Some(sibling),
                (Some(first), Some(second)) => Some(PaneNode::Split {
                    orientation,
                    ratio,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (None, None) => None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIDE: PaneRect = PaneRect {
        x: 0,
        y: 0,
        width: 160,
        height: 50,
    };

    #[test]
    fn recursive_split_changes_only_focused_region() {
        let mut layout = PaneLayout::default();
        let a = layout.focused();
        let b = layout
            .split(Orientation::Vertical, 5_000, WIDE, 20, 8)
            .unwrap();
        let before = layout.rectangles(WIDE, 20, 8);
        let c = layout
            .split(Orientation::Horizontal, 5_000, before[1].1, 20, 8)
            .unwrap();
        let after = layout.rectangles(WIDE, 20, 8);
        assert_eq!(
            after.iter().find(|(id, _)| *id == a).unwrap().1,
            before[0].1
        );
        let b_rect = after.iter().find(|(id, _)| *id == b).unwrap().1;
        let c_rect = after.iter().find(|(id, _)| *id == c).unwrap().1;
        assert_eq!(b_rect.x, c_rect.x);
        assert_eq!(b_rect.width, c_rect.width);
        assert_eq!(b_rect.y, before[1].1.y);
        assert_eq!(c_rect.y, before[1].1.y + b_rect.height);
        let d = layout
            .split(Orientation::Vertical, 5_000, c_rect, 20, 8)
            .unwrap();
        assert_eq!(layout.panes(), vec![a, b, c, d]);
        let final_rects = layout.rectangles(WIDE, 20, 8);
        assert_eq!(
            final_rects.iter().find(|(id, _)| *id == a).unwrap().1,
            before[0].1
        );
        assert_eq!(
            final_rects.iter().find(|(id, _)| *id == b).unwrap().1,
            b_rect
        );
    }

    #[test]
    fn close_promotes_sibling_subtree_and_single_keeps_focused_leaf() {
        let mut layout = PaneLayout::default();
        let a = layout.focused();
        let b = layout
            .split(Orientation::Vertical, 5_000, WIDE, 20, 8)
            .unwrap();
        let c = layout
            .split(
                Orientation::Horizontal,
                5_000,
                layout.rectangles(WIDE, 20, 8)[1].1,
                20,
                8,
            )
            .unwrap();
        let d = layout
            .split(
                Orientation::Vertical,
                5_000,
                layout.rectangles(WIDE, 20, 8)[2].1,
                20,
                8,
            )
            .unwrap();
        layout.close(c).unwrap();
        assert_eq!(layout.panes(), vec![a, b, d]);
        assert_eq!(layout.focused(), d);
        layout.only_focused();
        assert_eq!(layout.root(), &PaneNode::Leaf(d));
        assert_eq!(layout.pane_count(), 1);
        assert_eq!(layout.focused(), d);
    }

    #[test]
    fn focus_cycles_and_unknown_pane_is_rejected() {
        let mut layout = PaneLayout::default();
        let a = layout.focused();
        let b = layout
            .split(Orientation::Vertical, 5_000, WIDE, 20, 8)
            .unwrap();
        layout.focus_next(false);
        assert_eq!(layout.focused(), a);
        layout.focus_next(true);
        assert_eq!(layout.focused(), b);
        assert_eq!(layout.focus(999), Err(LayoutError::UnknownPane));
    }

    #[test]
    fn rejects_small_splits_without_mutating_tree() {
        let mut layout = PaneLayout::default();
        let before = layout.clone();
        assert_eq!(
            layout.split(
                Orientation::Vertical,
                5_000,
                PaneRect { width: 39, ..WIDE },
                20,
                8
            ),
            Err(LayoutError::InsufficientSpace)
        );
        assert_eq!(layout, before);
        assert_eq!(
            layout.close(layout.focused()),
            Err(LayoutError::CannotCloseOnlyPane)
        );
    }

    #[test]
    fn restores_a_closed_stable_pane_id_without_reusing_ids() {
        let mut layout = PaneLayout::default();
        let reference = layout
            .split(Orientation::Vertical, 5_000, WIDE, 20, 8)
            .unwrap();
        layout.close(1).unwrap();
        assert_eq!(layout.panes(), vec![reference]);
        layout
            .split_with_existing_id(Orientation::Horizontal, 5_000, WIDE, 20, 8, 1)
            .unwrap();
        assert_eq!(layout.panes(), vec![reference, 1]);
        assert_eq!(layout.focused(), 1);
        let before = layout.clone();
        assert_eq!(
            layout.split_with_existing_id(Orientation::Vertical, 5_000, WIDE, 20, 8, 1),
            Err(LayoutError::UnknownPane)
        );
        assert_eq!(layout, before);
    }

    #[test]
    fn focused_pane_resize_changes_the_nearest_divider_in_expected_direction() {
        let mut layout = PaneLayout::default();
        let first = layout.focused();
        let second = layout
            .split(Orientation::Vertical, 5_000, WIDE, 20, 8)
            .unwrap();
        layout.resize_focused_parent(true).unwrap();
        let PaneNode::Split { ratio, .. } = layout.root() else {
            panic!("split root expected");
        };
        assert_eq!(*ratio, 4_500, "focused second pane should grow to the left");
        layout.focus(first).unwrap();
        layout.resize_focused_parent(true).unwrap();
        let PaneNode::Split { ratio, .. } = layout.root() else {
            panic!("split root expected");
        };
        assert_eq!(*ratio, 5_000, "focused first pane should grow to the right");
        layout.only_focused();
        assert_eq!(
            layout.resize_focused_parent(true),
            Err(LayoutError::NoParentSplit)
        );
        assert_eq!(layout.focused(), first);
        assert!(second > first);
    }

    #[test]
    fn resizing_a_leaf_changes_only_its_immediate_parent_split() {
        let mut layout = PaneLayout::default();
        let a = layout.focused();
        let b = layout
            .split(Orientation::Vertical, 5_000, WIDE, 20, 8)
            .unwrap();
        let original = layout.root().clone();
        layout.resize_parent_split(b, 6_000).unwrap();
        let PaneNode::Split { ratio, .. } = layout.root() else {
            panic!("expected root split")
        };
        assert_eq!(*ratio, 6_000);
        assert_eq!(layout.panes(), vec![a, b]);
        assert_ne!(layout.root(), &original);
        assert_eq!(
            layout.resize_parent_split(999, 6_000),
            Err(LayoutError::UnknownPane)
        );
    }

    #[test]
    fn remapping_panes_preserves_tree_and_focus_and_rejects_duplicate_ids() {
        let mut layout = PaneLayout::default();
        let a = layout.focused();
        let b = layout
            .split(Orientation::Vertical, 5_000, WIDE, 20, 8)
            .unwrap();
        let c = layout
            .split(Orientation::Horizontal, 5_000, WIDE, 20, 8)
            .unwrap();
        let tree_before = layout.root().clone();
        assert_eq!(layout.focused(), c);

        layout.remap_panes(&[(a, 11), (b, 22), (c, 33)]).unwrap();
        assert_eq!(layout.panes(), vec![11, 22, 33]);
        assert_eq!(layout.focused(), 33);
        assert_ne!(layout.root(), &tree_before);
        assert_eq!(
            layout.split(Orientation::Vertical, 5_000, WIDE, 20, 8),
            Ok(34)
        );

        let mut unchanged = PaneLayout::default();
        let before = unchanged.clone();
        assert_eq!(
            unchanged.remap_panes(&[(1, 4), (1, 4)]),
            Err(LayoutError::UnknownPane)
        );
        assert_eq!(unchanged, before);
    }
}
