//! Pane Layout Engine — binary split tree for terminal multiplexing.
//!
//! F2 of G10 (Kill tmux). Manages pane arrangement via a binary tree where:
//! - **Leaf nodes** hold a `PaneId` (maps to a PTY in the pool)
//! - **Split nodes** hold a direction (H/V), a ratio (0.0–1.0), and two children
//!
//! Given a screen `Rect`, the tree recursively divides space to compute
//! each pane's rectangle. Supports split, close, resize, and directional navigation.

use super::pool::PaneId;
use ratatui::layout::Rect;
use serde::{Deserialize, Serialize};

/// Split direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Horizontal, // left | right
    Vertical,   // top / bottom
}

/// A node in the layout tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LayoutNode {
    /// A leaf — contains a single pane
    Leaf { pane: PaneId },
    /// A split — two children divided by direction and ratio
    Split {
        direction: Direction,
        /// Fraction of space given to the first child (0.0–1.0)
        ratio: f32,
        first: Box<LayoutNode>,
        second: Box<LayoutNode>,
    },
}

/// The layout tree with focus tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutTree {
    root: Option<LayoutNode>,
    focused: Option<PaneId>,
}

/// A pane's computed position and size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneRect {
    pub pane: PaneId,
    pub rect: Rect,
}

impl LayoutNode {
    /// Create a leaf node
    fn leaf(pane: PaneId) -> Self {
        Self::Leaf { pane }
    }

    /// Collect all pane IDs in this subtree (left-to-right / top-to-bottom order)
    fn pane_ids(&self) -> Vec<PaneId> {
        match self {
            Self::Leaf { pane } => vec![*pane],
            Self::Split { first, second, .. } => {
                let mut ids = first.pane_ids();
                ids.extend(second.pane_ids());
                ids
            }
        }
    }

    /// Count leaves in this subtree
    fn leaf_count(&self) -> usize {
        match self {
            Self::Leaf { .. } => 1,
            Self::Split { first, second, .. } => first.leaf_count() + second.leaf_count(),
        }
    }

    /// Check if this subtree contains a pane
    fn contains(&self, id: PaneId) -> bool {
        match self {
            Self::Leaf { pane } => *pane == id,
            Self::Split { first, second, .. } => first.contains(id) || second.contains(id),
        }
    }

    /// Compute pane rectangles by recursively dividing the available area
    fn compute_rects(&self, area: Rect, out: &mut Vec<PaneRect>) {
        // Don't compute for zero-area rects
        if area.width == 0 || area.height == 0 {
            return;
        }

        match self {
            Self::Leaf { pane } => {
                out.push(PaneRect { pane: *pane, rect: area });
            }
            Self::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let (first_area, second_area) = split_rect(area, *direction, *ratio);
                first.compute_rects(first_area, out);
                second.compute_rects(second_area, out);
            }
        }
    }

    /// Split a specific pane, replacing its leaf with a split node.
    /// Returns true if the pane was found and split.
    fn split_pane(
        &mut self,
        target: PaneId,
        new_pane: PaneId,
        direction: Direction,
        ratio: f32,
    ) -> bool {
        match self {
            Self::Leaf { pane } if *pane == target => {
                let old = Self::leaf(target);
                let new = Self::leaf(new_pane);
                *self = Self::Split {
                    direction,
                    ratio,
                    first: Box::new(old),
                    second: Box::new(new),
                };
                true
            }
            Self::Leaf { .. } => false,
            Self::Split { first, second, .. } => {
                first.split_pane(target, new_pane, direction, ratio)
                    || second.split_pane(target, new_pane, direction, ratio)
            }
        }
    }

    /// Remove a pane from the tree. Returns the sibling node if removal
    /// collapses a split, or None if the pane wasn't found.
    fn remove_pane(&mut self, target: PaneId) -> Option<LayoutNode> {
        match self {
            Self::Leaf { pane } if *pane == target => {
                // Can't remove the root leaf from within — caller handles this
                None
            }
            Self::Leaf { .. } => None,
            Self::Split { first, second, .. } => {
                // Check if first child is the target leaf
                if matches!(first.as_ref(), Self::Leaf { pane } if *pane == target) {
                    return Some(*second.clone());
                }
                // Check if second child is the target leaf
                if matches!(second.as_ref(), Self::Leaf { pane } if *pane == target) {
                    return Some(*first.clone());
                }
                // Recurse into children
                if let Some(replacement) = first.remove_pane(target) {
                    *first = Box::new(replacement);
                    return Some(self.clone());
                }
                if let Some(replacement) = second.remove_pane(target) {
                    *second = Box::new(replacement);
                    return Some(self.clone());
                }
                None
            }
        }
    }

    /// Find the pane rect for a given pane ID within an area
    fn find_pane_rect(&self, target: PaneId, area: Rect) -> Option<Rect> {
        match self {
            Self::Leaf { pane } if *pane == target => Some(area),
            Self::Leaf { .. } => None,
            Self::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let (first_area, second_area) = split_rect(area, *direction, *ratio);
                first
                    .find_pane_rect(target, first_area)
                    .or_else(|| second.find_pane_rect(target, second_area))
            }
        }
    }

    /// Adjust the ratio of the split that directly contains the target pane.
    /// `delta` is added to the ratio (positive = grow first child).
    fn adjust_ratio(&mut self, target: PaneId, delta: f32) -> bool {
        match self {
            Self::Leaf { .. } => false,
            Self::Split {
                first,
                second,
                ratio,
                ..
            } => {
                // If either direct child contains the target, adjust this split
                if first.contains(target) || second.contains(target) {
                    let new_ratio = (*ratio + delta).clamp(0.1, 0.9);
                    *ratio = new_ratio;
                    return true;
                }
                // Otherwise recurse
                first.adjust_ratio(target, delta)
                    || second.adjust_ratio(target, delta)
            }
        }
    }
}

impl LayoutTree {
    /// Create an empty layout
    pub fn new() -> Self {
        Self {
            root: None,
            focused: None,
        }
    }

    /// Create a layout with a single pane
    pub fn with_pane(pane: PaneId) -> Self {
        Self {
            root: Some(LayoutNode::leaf(pane)),
            focused: Some(pane),
        }
    }

    /// Add the first pane (when tree is empty)
    pub fn add_first(&mut self, pane: PaneId) {
        if self.root.is_none() {
            self.root = Some(LayoutNode::leaf(pane));
            self.focused = Some(pane);
        }
    }

    /// Split the focused pane (or a specific pane) in the given direction.
    /// Returns the ID of the new pane's slot (caller must spawn the PTY).
    pub fn split(
        &mut self,
        target: PaneId,
        new_pane: PaneId,
        direction: Direction,
    ) -> bool {
        self.split_with_ratio(target, new_pane, direction, 0.5)
    }

    /// Split with a custom ratio
    pub fn split_with_ratio(
        &mut self,
        target: PaneId,
        new_pane: PaneId,
        direction: Direction,
        ratio: f32,
    ) -> bool {
        let ratio = ratio.clamp(0.1, 0.9);
        match &mut self.root {
            None => false,
            Some(root) => {
                let ok = root.split_pane(target, new_pane, direction, ratio);
                if ok {
                    self.focused = Some(new_pane);
                }
                ok
            }
        }
    }

    /// Close a pane, removing it from the layout.
    /// If it was the last pane, the tree becomes empty.
    /// Returns the pane ID that should receive focus after removal.
    pub fn close(&mut self, target: PaneId) -> Option<PaneId> {
        let root = self.root.take()?;

        // If root is the target leaf, tree becomes empty
        if matches!(&root, LayoutNode::Leaf { pane } if *pane == target) {
            self.focused = None;
            return None;
        }

        // Try to remove from tree
        let mut root = root;
        if let Some(replacement) = root.remove_pane(target) {
            self.root = Some(replacement);
        } else {
            // Pane not found — restore root
            self.root = Some(root);
            return self.focused;
        }

        // Update focus if the closed pane was focused
        if self.focused == Some(target) {
            let ids = self.pane_ids();
            self.focused = ids.first().copied();
        }

        self.focused
    }

    /// Compute all pane rectangles for the given total area
    pub fn compute_rects(&self, area: Rect) -> Vec<PaneRect> {
        let mut rects = Vec::new();
        if let Some(root) = &self.root {
            root.compute_rects(area, &mut rects);
        }
        rects
    }

    /// Get the rect for a specific pane
    pub fn pane_rect(&self, pane: PaneId, area: Rect) -> Option<Rect> {
        self.root.as_ref()?.find_pane_rect(pane, area)
    }

    /// Get the focused pane
    pub fn focused(&self) -> Option<PaneId> {
        self.focused
    }

    /// Set focus to a specific pane
    pub fn focus(&mut self, pane: PaneId) -> bool {
        if self.root.as_ref().is_some_and(|r| r.contains(pane)) {
            self.focused = Some(pane);
            true
        } else {
            false
        }
    }

    /// Get all pane IDs in layout order
    pub fn pane_ids(&self) -> Vec<PaneId> {
        self.root.as_ref().map_or_else(Vec::new, |r| r.pane_ids())
    }

    /// Get pane count
    pub fn len(&self) -> usize {
        self.root.as_ref().map_or(0, |r| r.leaf_count())
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    /// Navigate focus in a direction within the given area.
    /// Finds the pane whose center is closest in the given direction
    /// from the currently focused pane.
    pub fn navigate(&mut self, dir: Direction, area: Rect) -> Option<PaneId> {
        let focused = self.focused?;
        let rects = self.compute_rects(area);
        let current = rects.iter().find(|r| r.pane == focused)?;

        let cx = current.rect.x as i32 + current.rect.width as i32 / 2;
        let cy = current.rect.y as i32 + current.rect.height as i32 / 2;

        let mut best: Option<(PaneId, i32)> = None;

        for pr in &rects {
            if pr.pane == focused {
                continue;
            }
            let px = pr.rect.x as i32 + pr.rect.width as i32 / 2;
            let py = pr.rect.y as i32 + pr.rect.height as i32 / 2;

            let valid = match dir {
                Direction::Horizontal => px > cx, // right
                Direction::Vertical => py > cy,   // down
            };

            if !valid {
                continue;
            }

            let dist = (px - cx).abs() + (py - cy).abs();
            if best.is_none() || dist < best.unwrap().1 {
                best = Some((pr.pane, dist));
            }
        }

        if let Some((pane, _)) = best {
            self.focused = Some(pane);
            Some(pane)
        } else {
            None
        }
    }

    /// Navigate to the pane in the opposite direction (left / up)
    pub fn navigate_back(&mut self, dir: Direction, area: Rect) -> Option<PaneId> {
        let focused = self.focused?;
        let rects = self.compute_rects(area);
        let current = rects.iter().find(|r| r.pane == focused)?;

        let cx = current.rect.x as i32 + current.rect.width as i32 / 2;
        let cy = current.rect.y as i32 + current.rect.height as i32 / 2;

        let mut best: Option<(PaneId, i32)> = None;

        for pr in &rects {
            if pr.pane == focused {
                continue;
            }
            let px = pr.rect.x as i32 + pr.rect.width as i32 / 2;
            let py = pr.rect.y as i32 + pr.rect.height as i32 / 2;

            let valid = match dir {
                Direction::Horizontal => px < cx, // left
                Direction::Vertical => py < cy,   // up
            };

            if !valid {
                continue;
            }

            let dist = (px - cx).abs() + (py - cy).abs();
            if best.is_none() || dist < best.unwrap().1 {
                best = Some((pr.pane, dist));
            }
        }

        if let Some((pane, _)) = best {
            self.focused = Some(pane);
            Some(pane)
        } else {
            None
        }
    }

    /// Cycle focus to the next pane (wrapping)
    pub fn focus_next(&mut self) -> Option<PaneId> {
        let ids = self.pane_ids();
        if ids.is_empty() {
            return None;
        }
        let focused = self.focused?;
        let idx = ids.iter().position(|&id| id == focused).unwrap_or(0);
        let next = ids[(idx + 1) % ids.len()];
        self.focused = Some(next);
        Some(next)
    }

    /// Cycle focus to the previous pane (wrapping)
    pub fn focus_prev(&mut self) -> Option<PaneId> {
        let ids = self.pane_ids();
        if ids.is_empty() {
            return None;
        }
        let focused = self.focused?;
        let idx = ids.iter().position(|&id| id == focused).unwrap_or(0);
        let prev = ids[(idx + ids.len() - 1) % ids.len()];
        self.focused = Some(prev);
        Some(prev)
    }

    /// Adjust the size of the focused pane's parent split.
    /// `delta` > 0 grows the focused pane, < 0 shrinks it.
    pub fn resize_focused(&mut self, delta: f32) -> bool {
        let focused = match self.focused {
            Some(f) => f,
            None => return false,
        };
        match &mut self.root {
            Some(root) => root.adjust_ratio(focused, delta),
            None => false,
        }
    }

    /// Serialize to JSON for persistence
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize from JSON
    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }
}

impl Default for LayoutTree {
    fn default() -> Self {
        Self::new()
    }
}

/// Split a Rect into two sub-rects based on direction and ratio.
/// Guarantees each sub-rect has at least 1 row/col.
fn split_rect(area: Rect, direction: Direction, ratio: f32) -> (Rect, Rect) {
    match direction {
        Direction::Horizontal => {
            let first_width = ((area.width as f32 * ratio).round() as u16).clamp(1, area.width.saturating_sub(1));
            let second_width = area.width.saturating_sub(first_width);
            (
                Rect::new(area.x, area.y, first_width, area.height),
                Rect::new(area.x + first_width, area.y, second_width, area.height),
            )
        }
        Direction::Vertical => {
            let first_height = ((area.height as f32 * ratio).round() as u16).clamp(1, area.height.saturating_sub(1));
            let second_height = area.height.saturating_sub(first_height);
            (
                Rect::new(area.x, area.y, area.width, first_height),
                Rect::new(area.x, area.y + first_height, area.width, second_height),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> Rect {
        Rect::new(0, 0, 120, 40)
    }

    #[test]
    fn test_empty_tree() {
        let tree = LayoutTree::new();
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
        assert!(tree.focused().is_none());
        assert!(tree.pane_ids().is_empty());
        assert!(tree.compute_rects(area()).is_empty());
    }

    #[test]
    fn test_single_pane() {
        let tree = LayoutTree::with_pane(1);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree.focused(), Some(1));
        assert_eq!(tree.pane_ids(), vec![1]);

        let rects = tree.compute_rects(area());
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].pane, 1);
        assert_eq!(rects[0].rect, area());
    }

    #[test]
    fn test_add_first() {
        let mut tree = LayoutTree::new();
        tree.add_first(5);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree.focused(), Some(5));
    }

    #[test]
    fn test_horizontal_split() {
        let mut tree = LayoutTree::with_pane(1);
        assert!(tree.split(1, 2, Direction::Horizontal));

        assert_eq!(tree.len(), 2);
        assert_eq!(tree.focused(), Some(2)); // focus moves to new pane
        assert_eq!(tree.pane_ids(), vec![1, 2]);

        let rects = tree.compute_rects(area());
        assert_eq!(rects.len(), 2);

        // Left pane: 60 cols
        assert_eq!(rects[0].pane, 1);
        assert_eq!(rects[0].rect.x, 0);
        assert_eq!(rects[0].rect.width, 60);
        assert_eq!(rects[0].rect.height, 40);

        // Right pane: 60 cols
        assert_eq!(rects[1].pane, 2);
        assert_eq!(rects[1].rect.x, 60);
        assert_eq!(rects[1].rect.width, 60);
        assert_eq!(rects[1].rect.height, 40);
    }

    #[test]
    fn test_vertical_split() {
        let mut tree = LayoutTree::with_pane(1);
        assert!(tree.split(1, 2, Direction::Vertical));

        let rects = tree.compute_rects(area());
        assert_eq!(rects.len(), 2);

        // Top pane: 20 rows
        assert_eq!(rects[0].pane, 1);
        assert_eq!(rects[0].rect.y, 0);
        assert_eq!(rects[0].rect.height, 20);

        // Bottom pane: 20 rows
        assert_eq!(rects[1].pane, 2);
        assert_eq!(rects[1].rect.y, 20);
        assert_eq!(rects[1].rect.height, 20);
    }

    #[test]
    fn test_nested_split() {
        let mut tree = LayoutTree::with_pane(1);
        // Split 1 horizontally → [1 | 2]
        tree.split(1, 2, Direction::Horizontal);
        // Split 2 vertically → [1 | (2 / 3)]
        tree.split(2, 3, Direction::Vertical);

        assert_eq!(tree.len(), 3);
        assert_eq!(tree.pane_ids(), vec![1, 2, 3]);

        let rects = tree.compute_rects(area());
        assert_eq!(rects.len(), 3);

        // Pane 1: left half (60 cols, 40 rows)
        assert_eq!(rects[0].pane, 1);
        assert_eq!(rects[0].rect.width, 60);
        assert_eq!(rects[0].rect.height, 40);

        // Pane 2: top-right (60 cols, 20 rows)
        assert_eq!(rects[1].pane, 2);
        assert_eq!(rects[1].rect.x, 60);
        assert_eq!(rects[1].rect.width, 60);
        assert_eq!(rects[1].rect.height, 20);

        // Pane 3: bottom-right (60 cols, 20 rows)
        assert_eq!(rects[2].pane, 3);
        assert_eq!(rects[2].rect.x, 60);
        assert_eq!(rects[2].rect.y, 20);
        assert_eq!(rects[2].rect.height, 20);
    }

    #[test]
    fn test_split_nonexistent() {
        let mut tree = LayoutTree::with_pane(1);
        assert!(!tree.split(99, 2, Direction::Horizontal));
        assert_eq!(tree.len(), 1);
    }

    #[test]
    fn test_split_with_ratio() {
        let mut tree = LayoutTree::with_pane(1);
        tree.split_with_ratio(1, 2, Direction::Horizontal, 0.3);

        let rects = tree.compute_rects(area());
        // 120 * 0.3 = 36
        assert_eq!(rects[0].rect.width, 36);
        assert_eq!(rects[1].rect.width, 84);
    }

    #[test]
    fn test_ratio_clamped() {
        let mut tree = LayoutTree::with_pane(1);
        // Ratio too small — clamped to 0.1
        tree.split_with_ratio(1, 2, Direction::Horizontal, 0.01);
        let rects = tree.compute_rects(area());
        assert!(rects[0].rect.width >= 1);
        assert!(rects[1].rect.width >= 1);
    }

    #[test]
    fn test_close_pane() {
        let mut tree = LayoutTree::with_pane(1);
        tree.split(1, 2, Direction::Horizontal);
        tree.split(2, 3, Direction::Vertical);

        // Close pane 2 → [1 | 3]
        tree.focus(2);
        let next = tree.close(2);
        assert!(next.is_some());
        assert_eq!(tree.len(), 2);
        assert_eq!(tree.pane_ids(), vec![1, 3]);
    }

    #[test]
    fn test_close_last_pane() {
        let mut tree = LayoutTree::with_pane(1);
        let next = tree.close(1);
        assert!(next.is_none());
        assert!(tree.is_empty());
    }

    #[test]
    fn test_close_focused_moves_focus() {
        let mut tree = LayoutTree::with_pane(1);
        tree.split(1, 2, Direction::Horizontal);
        tree.focus(2);

        tree.close(2);
        assert_eq!(tree.focused(), Some(1));
    }

    #[test]
    fn test_focus_set() {
        let mut tree = LayoutTree::with_pane(1);
        tree.split(1, 2, Direction::Horizontal);

        assert!(tree.focus(1));
        assert_eq!(tree.focused(), Some(1));

        assert!(tree.focus(2));
        assert_eq!(tree.focused(), Some(2));

        assert!(!tree.focus(99)); // nonexistent
        assert_eq!(tree.focused(), Some(2)); // unchanged
    }

    #[test]
    fn test_focus_next_prev() {
        let mut tree = LayoutTree::with_pane(1);
        tree.split(1, 2, Direction::Horizontal);
        tree.split(2, 3, Direction::Horizontal);
        tree.focus(1);

        assert_eq!(tree.focus_next(), Some(2));
        assert_eq!(tree.focus_next(), Some(3));
        assert_eq!(tree.focus_next(), Some(1)); // wraps

        assert_eq!(tree.focus_prev(), Some(3)); // wraps back
        assert_eq!(tree.focus_prev(), Some(2));
    }

    #[test]
    fn test_navigate_horizontal() {
        let mut tree = LayoutTree::with_pane(1);
        tree.split(1, 2, Direction::Horizontal);
        tree.focus(1);

        // Navigate right → pane 2
        let next = tree.navigate(Direction::Horizontal, area());
        assert_eq!(next, Some(2));
        assert_eq!(tree.focused(), Some(2));

        // Navigate left → pane 1
        let prev = tree.navigate_back(Direction::Horizontal, area());
        assert_eq!(prev, Some(1));
    }

    #[test]
    fn test_navigate_vertical() {
        let mut tree = LayoutTree::with_pane(1);
        tree.split(1, 2, Direction::Vertical);
        tree.focus(1);

        // Navigate down → pane 2
        let next = tree.navigate(Direction::Vertical, area());
        assert_eq!(next, Some(2));

        // Navigate up → pane 1
        let prev = tree.navigate_back(Direction::Vertical, area());
        assert_eq!(prev, Some(1));
    }

    #[test]
    fn test_navigate_no_pane_in_direction() {
        let mut tree = LayoutTree::with_pane(1);
        tree.split(1, 2, Direction::Horizontal);
        tree.focus(2); // rightmost

        // Navigate right — nothing there
        assert_eq!(tree.navigate(Direction::Horizontal, area()), None);
        assert_eq!(tree.focused(), Some(2)); // unchanged
    }

    #[test]
    fn test_resize_focused() {
        let mut tree = LayoutTree::with_pane(1);
        tree.split(1, 2, Direction::Horizontal);
        tree.focus(1);

        // Grow pane 1 by 10%
        assert!(tree.resize_focused(0.1));

        let rects = tree.compute_rects(area());
        // 120 * 0.6 = 72
        assert_eq!(rects[0].rect.width, 72);
        assert_eq!(rects[1].rect.width, 48);
    }

    #[test]
    fn test_resize_clamped() {
        let mut tree = LayoutTree::with_pane(1);
        tree.split(1, 2, Direction::Horizontal);
        tree.focus(1);

        // Try to grow way past max — clamped to 0.9
        tree.resize_focused(10.0);
        let rects = tree.compute_rects(area());
        assert!(rects[0].rect.width <= 108); // 120 * 0.9
        assert!(rects[1].rect.width >= 12);  // 120 * 0.1
    }

    #[test]
    fn test_pane_rect() {
        let mut tree = LayoutTree::with_pane(1);
        tree.split(1, 2, Direction::Horizontal);

        let rect = tree.pane_rect(1, area());
        assert!(rect.is_some());
        assert_eq!(rect.unwrap().width, 60);

        assert!(tree.pane_rect(99, area()).is_none());
    }

    #[test]
    fn test_serialize_deserialize() {
        let mut tree = LayoutTree::with_pane(1);
        tree.split(1, 2, Direction::Horizontal);
        tree.split(2, 3, Direction::Vertical);

        let json = tree.to_json().unwrap();
        let restored = LayoutTree::from_json(&json).unwrap();

        assert_eq!(restored.len(), 3);
        assert_eq!(restored.pane_ids(), vec![1, 2, 3]);
        assert_eq!(restored.focused(), tree.focused());
    }

    #[test]
    fn test_split_rect_horizontal() {
        let r = Rect::new(0, 0, 100, 50);
        let (a, b) = split_rect(r, Direction::Horizontal, 0.5);
        assert_eq!(a.width + b.width, 100);
        assert_eq!(a.x, 0);
        assert_eq!(b.x, a.width);
        assert_eq!(a.height, 50);
        assert_eq!(b.height, 50);
    }

    #[test]
    fn test_split_rect_vertical() {
        let r = Rect::new(0, 0, 100, 50);
        let (a, b) = split_rect(r, Direction::Vertical, 0.5);
        assert_eq!(a.height + b.height, 50);
        assert_eq!(a.y, 0);
        assert_eq!(b.y, a.height);
        assert_eq!(a.width, 100);
    }

    #[test]
    fn test_split_rect_minimum_size() {
        // Very small rect — each side should get at least 1
        let r = Rect::new(0, 0, 2, 2);
        let (a, b) = split_rect(r, Direction::Horizontal, 0.5);
        assert!(a.width >= 1);
        assert!(b.width >= 1);
    }

    #[test]
    fn test_four_pane_grid() {
        // Classic 2x2 grid: split H, then split each half V
        let mut tree = LayoutTree::with_pane(1);
        tree.split(1, 2, Direction::Horizontal);  // [1 | 2]
        tree.split(1, 3, Direction::Vertical);     // [(1/3) | 2]
        tree.split(2, 4, Direction::Vertical);     // [(1/3) | (2/4)]

        assert_eq!(tree.len(), 4);
        let rects = tree.compute_rects(area());
        assert_eq!(rects.len(), 4);

        // All 4 panes should have non-zero area
        for pr in &rects {
            assert!(pr.rect.width > 0);
            assert!(pr.rect.height > 0);
        }

        // Total area should be covered (no gaps)
        let total_area: u32 = rects
            .iter()
            .map(|pr| pr.rect.width as u32 * pr.rect.height as u32)
            .sum();
        assert_eq!(total_area, 120 * 40);
    }

    #[test]
    fn test_deep_nesting() {
        let mut tree = LayoutTree::with_pane(1);
        for i in 2..=8 {
            let dir = if i % 2 == 0 {
                Direction::Horizontal
            } else {
                Direction::Vertical
            };
            tree.split(i - 1, i, dir);
        }

        assert_eq!(tree.len(), 8);
        let rects = tree.compute_rects(area());
        assert_eq!(rects.len(), 8);

        // All panes visible (non-zero)
        for pr in &rects {
            assert!(pr.rect.width > 0, "Pane {} has zero width", pr.pane);
            assert!(pr.rect.height > 0, "Pane {} has zero height", pr.pane);
        }
    }
}
