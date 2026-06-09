use driftwm::window_ext::WindowExt;
use smithay::desktop::Window;
use smithay::reexports::wayland_server::{Resource, backend::ObjectId};
use smithay::utils::{Logical, Point, Rectangle, Size};
use smithay::wayland::seat::WaylandFocus;
use std::time::Instant;

use super::{
    DriftWm, FocusTarget, WorkspaceId,
    workspace::{DwindleNode, DwindleNodeId, DwindleSplit, WorkspaceState},
};

const MIN_DWINDLE_TILE_WIDTH: i32 = 600;
const MIN_DWINDLE_TILE_HEIGHT: i32 = 360;

pub(crate) fn remove_dwindle_window(workspace: &mut WorkspaceState, id: &ObjectId) {
    let Some(leaf_id) = find_dwindle_leaf(workspace, id) else {
        return;
    };

    let parent_id = workspace
        .dwindle_nodes
        .get(&leaf_id)
        .and_then(|node| node.parent);
    let Some(parent_id) = parent_id else {
        workspace.dwindle_nodes.remove(&leaf_id);
        workspace.dwindle_root = None;
        return;
    };

    let Some(parent) = workspace.dwindle_nodes.get(&parent_id).cloned() else {
        workspace.dwindle_nodes.remove(&leaf_id);
        return;
    };
    let Some((left, right)) = parent.children else {
        workspace.dwindle_nodes.remove(&leaf_id);
        return;
    };
    let sibling_id = if left == leaf_id { right } else { left };
    let grandparent_id = parent.parent;

    if let Some(grandparent_id) = grandparent_id {
        if let Some(grandparent) = workspace.dwindle_nodes.get_mut(&grandparent_id)
            && let Some((gleft, gright)) = grandparent.children
        {
            grandparent.children = Some(if gleft == parent_id {
                (sibling_id, gright)
            } else {
                (gleft, sibling_id)
            });
        }
        if let Some(sibling) = workspace.dwindle_nodes.get_mut(&sibling_id) {
            sibling.parent = Some(grandparent_id);
        }
    } else {
        workspace.dwindle_root = Some(sibling_id);
        if let Some(sibling) = workspace.dwindle_nodes.get_mut(&sibling_id) {
            sibling.parent = None;
        }
    }

    workspace.dwindle_nodes.remove(&leaf_id);
    workspace.dwindle_nodes.remove(&parent_id);
}

fn find_dwindle_leaf(workspace: &WorkspaceState, id: &ObjectId) -> Option<DwindleNodeId> {
    workspace.dwindle_nodes.iter().find_map(|(node_id, node)| {
        (node.children.is_none() && node.window.as_ref() == Some(id)).then_some(*node_id)
    })
}

fn find_dwindle_leaf_at_point(
    workspace: &WorkspaceState,
    point: Point<i32, Logical>,
) -> Option<DwindleNodeId> {
    workspace.tile_rects.iter().find_map(|(id, rect)| {
        rect.contains(point)
            .then(|| find_dwindle_leaf(workspace, id))
            .flatten()
    })
}

fn split_from_rect(rect: Rectangle<i32, Logical>, gap: i32) -> DwindleSplit {
    let can_vertical = rect.size.w - gap >= MIN_DWINDLE_TILE_WIDTH * 2;
    let can_horizontal = rect.size.h - gap >= MIN_DWINDLE_TILE_HEIGHT * 2;
    if can_vertical && (!can_horizontal || rect.size.w >= rect.size.h) {
        DwindleSplit::Vertical
    } else {
        DwindleSplit::Horizontal
    }
}

fn rect_can_dwindle_split(rect: Rectangle<i32, Logical>, gap: i32) -> bool {
    (rect.size.w - gap >= MIN_DWINDLE_TILE_WIDTH * 2)
        || (rect.size.h - gap >= MIN_DWINDLE_TILE_HEIGHT * 2)
}

fn split_rect(
    rect: Rectangle<i32, Logical>,
    gap: i32,
    split: DwindleSplit,
    ratio: f32,
) -> (Rectangle<i32, Logical>, Rectangle<i32, Logical>) {
    let ratio = ratio.clamp(0.15, 0.85);
    match split {
        DwindleSplit::Vertical => {
            let available = (rect.size.w - gap).max(2);
            let first_w = ((available as f32) * ratio).round() as i32;
            let second_w = available - first_w;
            (
                Rectangle::new(rect.loc, Size::from((first_w, rect.size.h))),
                Rectangle::new(
                    Point::from((rect.loc.x + first_w + gap, rect.loc.y)),
                    Size::from((second_w, rect.size.h)),
                ),
            )
        }
        DwindleSplit::Horizontal => {
            let available = (rect.size.h - gap).max(2);
            let first_h = ((available as f32) * ratio).round() as i32;
            let second_h = available - first_h;
            (
                Rectangle::new(rect.loc, Size::from((rect.size.w, first_h))),
                Rectangle::new(
                    Point::from((rect.loc.x, rect.loc.y + first_h + gap)),
                    Size::from((rect.size.w, second_h)),
                ),
            )
        }
    }
}

fn calculate_dwindle_rects(
    workspace: &WorkspaceState,
    node_id: DwindleNodeId,
    rect: Rectangle<i32, Logical>,
    gap: i32,
    out: &mut std::collections::HashMap<ObjectId, Rectangle<i32, Logical>>,
) {
    let Some(node) = workspace.dwindle_nodes.get(&node_id) else {
        return;
    };
    if let Some(id) = node.window.as_ref() {
        out.insert(id.clone(), rect);
        return;
    }

    let Some((left, right)) = node.children else {
        return;
    };
    let (left_rect, right_rect) = split_rect(rect, gap, node.split, node.ratio);
    calculate_dwindle_rects(workspace, left, left_rect, gap, out);
    calculate_dwindle_rects(workspace, right, right_rect, gap, out);
}

fn insert_dwindle_window(
    workspace: &mut WorkspaceState,
    id: ObjectId,
    anchor_id: Option<ObjectId>,
    pointer: Option<Point<i32, Logical>>,
    gap: i32,
) {
    if find_dwindle_leaf(workspace, &id).is_some() {
        return;
    }

    let new_leaf = DwindleNode {
        parent: None,
        window: Some(id.clone()),
        children: None,
        split: DwindleSplit::Vertical,
        ratio: 0.5,
    };

    let Some(root_id) = workspace.dwindle_root else {
        let node_id = workspace.alloc_dwindle_node(new_leaf);
        workspace.dwindle_root = Some(node_id);
        return;
    };

    let requested_anchor_leaf = anchor_id
        .as_ref()
        .and_then(|id| find_dwindle_leaf(workspace, id))
        .or_else(|| pointer.and_then(|point| find_dwindle_leaf_at_point(workspace, point)))
        .or_else(|| {
            workspace
                .tile_rects
                .iter()
                .max_by_key(|(_, rect)| rect.size.w.saturating_mul(rect.size.h))
                .and_then(|(id, _)| find_dwindle_leaf(workspace, id))
        })
        .unwrap_or(root_id);

    let requested_anchor_rect = workspace
        .dwindle_nodes
        .get(&requested_anchor_leaf)
        .and_then(|node| node.window.as_ref())
        .and_then(|id| workspace.tile_rects.get(id).cloned())
        .unwrap_or(workspace.rect);
    let anchor_leaf = if rect_can_dwindle_split(requested_anchor_rect, gap) {
        requested_anchor_leaf
    } else {
        workspace
            .tile_rects
            .iter()
            .filter(|(_, rect)| rect_can_dwindle_split(**rect, gap))
            .max_by_key(|(_, rect)| rect.size.w.saturating_mul(rect.size.h))
            .and_then(|(id, _)| find_dwindle_leaf(workspace, id))
            .unwrap_or(requested_anchor_leaf)
    };

    let anchor_rect = workspace
        .dwindle_nodes
        .get(&anchor_leaf)
        .and_then(|node| node.window.as_ref())
        .and_then(|id| workspace.tile_rects.get(id).cloned())
        .unwrap_or(workspace.rect);
    let split = split_from_rect(anchor_rect, gap);
    let parent_of_anchor = workspace
        .dwindle_nodes
        .get(&anchor_leaf)
        .and_then(|node| node.parent);

    let new_leaf_id = workspace.alloc_dwindle_node(new_leaf);
    let pointer_first = pointer.is_some_and(|point| match split {
        DwindleSplit::Vertical => point.x < anchor_rect.loc.x + anchor_rect.size.w / 2,
        DwindleSplit::Horizontal => point.y < anchor_rect.loc.y + anchor_rect.size.h / 2,
    });
    let children = if pointer_first {
        (new_leaf_id, anchor_leaf)
    } else {
        (anchor_leaf, new_leaf_id)
    };

    let parent_id = workspace.alloc_dwindle_node(DwindleNode {
        parent: parent_of_anchor,
        window: None,
        children: Some(children),
        split,
        ratio: 0.5,
    });

    if let Some(anchor) = workspace.dwindle_nodes.get_mut(&anchor_leaf) {
        anchor.parent = Some(parent_id);
    }
    if let Some(new) = workspace.dwindle_nodes.get_mut(&new_leaf_id) {
        new.parent = Some(parent_id);
    }

    if let Some(old_parent_id) = parent_of_anchor {
        if let Some(old_parent) = workspace.dwindle_nodes.get_mut(&old_parent_id)
            && let Some((left, right)) = old_parent.children
        {
            old_parent.children = Some(if left == anchor_leaf {
                (parent_id, right)
            } else {
                (left, parent_id)
            });
        }
    } else {
        workspace.dwindle_root = Some(parent_id);
    }
}

#[derive(Clone, Debug)]
pub struct TiledUnmapState {
    workspace_id: WorkspaceId,
    removed_rect: Option<Rectangle<i32, Logical>>,
}

impl DriftWm {
    fn window_object_id(window: &Window) -> Option<ObjectId> {
        window.wl_surface().map(|surface| surface.id())
    }

    fn hovered_or_focused_window(&self) -> Option<Window> {
        if let Some(pointer) = self.seat.get_pointer() {
            let pos = pointer.current_location();
            if let Some((window, _)) = self.space.element_under(pos) {
                let window = window.clone();
                if !window.is_widget() {
                    return Some(window);
                }
            }
        }
        self.focused_window().filter(|w| !w.is_widget())
    }

    fn purge_workspace_tile_state(&mut self, id: &ObjectId, remove_membership: bool) {
        self.tile_rects.remove(id);
        for workspace in self.workspaces.values_mut() {
            workspace.tile_rects.remove(id);
            if remove_membership {
                workspace.windows.remove(id);
                remove_dwindle_window(workspace, id);
            }
        }
    }

    fn focus_nearest_tiled_after_unmap(
        &mut self,
        workspace_id: WorkspaceId,
        removed_rect: Rectangle<i32, Logical>,
    ) {
        let removed_center = Point::<i32, Logical>::from((
            removed_rect.loc.x + removed_rect.size.w / 2,
            removed_rect.loc.y + removed_rect.size.h / 2,
        ));
        let Some(workspace) = self.workspaces.get(&workspace_id) else {
            return;
        };
        let Some((candidate_id, _)) = workspace.tile_rects.iter().min_by_key(|(_, rect)| {
            let center_x = rect.loc.x + rect.size.w / 2;
            let center_y = rect.loc.y + rect.size.h / 2;
            let dx = center_x - removed_center.x;
            let dy = center_y - removed_center.y;
            dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy))
        }) else {
            crate::diagnostics::log(format!(
                "tile:focus_after_unmap workspace={workspace_id} skip=no_candidate"
            ));
            return;
        };
        let candidate_id = candidate_id.clone();
        let candidate = self
            .space
            .elements()
            .find(|window| {
                Self::window_object_id(window)
                    .as_ref()
                    .is_some_and(|id| *id == candidate_id)
            })
            .cloned();
        let Some(candidate) = candidate else {
            crate::diagnostics::log(format!(
                "tile:focus_after_unmap workspace={workspace_id} skip=candidate_unmapped id={candidate_id:?}"
            ));
            return;
        };

        crate::diagnostics::log(format!(
            "tile:focus_after_unmap workspace={workspace_id} id={candidate_id:?} app={:?} title={:?}",
            candidate.app_id_or_class(),
            candidate.window_title()
        ));
        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        self.raise_and_focus(&candidate, serial);
    }

    fn workspace_for_window_id(&self, id: &ObjectId) -> Option<WorkspaceId> {
        self.workspaces
            .iter()
            .find_map(|(&workspace_id, workspace)| {
                workspace.windows.contains(id).then_some(workspace_id)
            })
    }

    fn snap_window_to_existing_tile(&mut self, window: &Window, id: &ObjectId) -> bool {
        let Some(rect) = self.tile_rects.get(id).cloned().or_else(|| {
            self.workspaces
                .values()
                .find_map(|workspace| workspace.tile_rects.get(id).cloned())
        }) else {
            return false;
        };

        crate::diagnostics::log(format!(
            "tile:snap_existing id={id:?} loc=({}, {}) size={}x{}",
            rect.loc.x, rect.loc.y, rect.size.w, rect.size.h
        ));
        if let Some(toplevel) = window.toplevel() {
            crate::diagnostics::log(format!("tile:snap_existing_configure_begin id={id:?}"));
            crate::handlers::set_tiled_states(&toplevel);
            toplevel.with_pending_state(|state| {
                state.size = Some(rect.size);
            });
            toplevel.send_configure();
            crate::diagnostics::log(format!("tile:snap_existing_configure_done id={id:?}"));
        }
        crate::diagnostics::log(format!("tile:snap_existing_map_begin id={id:?}"));
        self.space.map_element(window.clone(), rect.loc, false);
        crate::diagnostics::log(format!("tile:snap_existing_map_done id={id:?}"));
        self.mark_all_dirty();
        crate::diagnostics::log(format!("tile:snap_existing_done id={id:?}"));
        true
    }

    fn is_tiling_candidate(&self, window: &Window) -> bool {
        if window.x11_surface().is_some() && Self::x11_window_should_float(window) {
            return false;
        }
        // A raw XDG parent alone is not enough to float. GTK/GNOME apps can
        // expose parent metadata on normal toplevels; modal/dialog intent is
        // handled separately by is_modal(), while X11 transients still use the
        // stricter X11 float path above.
        if window.is_widget() || window.is_modal() {
            return false;
        }
        if self.fullscreen.values().any(|fs| fs.window == *window) {
            return false;
        }
        Self::window_object_id(window).is_some_and(|id| !self.floating_windows.contains(&id))
    }

    pub fn toggle_floating_window(&mut self) {
        let Some(window) = self.focused_window().filter(|w| !w.is_widget()) else {
            return;
        };
        let Some(id) = Self::window_object_id(&window) else {
            return;
        };

        if self.floating_windows.remove(&id) {
            crate::diagnostics::log(format!("tile:toggle_floating_to_tiled id={id:?}"));
            // Hyprland-style transition: toggling back to tiled reinserts the
            // window into the layout tree, rather than reusing stale geometry.
            let workspace = self
                .workspace_for_window(&window)
                .or_else(|| self.workspace_at_pointer())
                .unwrap_or(self.active_workspace);
            self.purge_workspace_tile_state(&id, true);
            self.assign_window_to_workspace(id.clone(), workspace);
            self.active_workspace = workspace;
            self.tile_workspace(workspace, false);
            self.stabilize_tiled_workspace_view();
        } else {
            crate::diagnostics::log(format!("tile:toggle_floating_to_float id={id:?}"));
            self.floating_windows.insert(id.clone());
            let source_workspace = self
                .workspace_for_window_id(&id)
                .or_else(|| self.workspace_for_window(&window))
                .unwrap_or(self.active_workspace);
            // Remove it from the tiled membership so remaining windows expand.
            self.purge_workspace_tile_state(&id, true);
            self.tile_workspace(source_workspace, false);
            self.active_workspace = source_workspace;
            self.stabilize_tiled_workspace_view();

            let usable = self.get_usable_area();
            let camera = self.camera().to_i32_round::<i32>();
            let width = (usable.size.w as f64 * 0.55).round() as i32;
            let height = (usable.size.h as f64 * 0.55).round() as i32;
            let size = Size::<i32, Logical>::from((width.max(480), height.max(320)));
            let loc = Point::<i32, Logical>::from((
                camera.x + usable.loc.x + (usable.size.w - size.w) / 2,
                camera.y + usable.loc.y + (usable.size.h - size.h) / 2,
            ));

            if let Some(toplevel) = window.toplevel() {
                crate::handlers::unset_tiled_states(&toplevel);
                toplevel.with_pending_state(|state| {
                    state.size = Some(size);
                });
                toplevel.send_configure();
            }
            self.space.map_element(window.clone(), loc, true);
            if let Some(surface) = window.wl_surface() {
                let serial = smithay::utils::SERIAL_COUNTER.next_serial();
                self.seat.get_keyboard().unwrap().set_focus(
                    self,
                    Some(FocusTarget(surface.into_owned())),
                    serial,
                );
            }
        }
    }

    pub fn workspace_at_pointer(&self) -> Option<WorkspaceId> {
        let pointer = self.seat.get_pointer()?;
        self.workspace_at_point(pointer.current_location().to_i32_round::<i32>())
    }

    pub fn in_workspace_perspective(&self) -> bool {
        if self.zoom() < 0.95 {
            return false;
        }
        let camera = self.camera();
        let screen_center = self.usable_center_screen();
        let zoom = self.zoom();
        let center = Point::<i32, Logical>::from((
            (camera.x + screen_center.x / zoom).round() as i32,
            (camera.y + screen_center.y / zoom).round() as i32,
        ));
        self.workspace_at_point(center) == Some(self.active_workspace)
    }

    pub fn move_window_to_workspace(&mut self, workspace_id: WorkspaceId) {
        let Some(window) = self.hovered_or_focused_window() else {
            return;
        };
        let Some(id) = Self::window_object_id(&window) else {
            return;
        };
        let Some(workspace_rect) = self.workspaces.get(&workspace_id).map(|w| w.rect) else {
            tracing::warn!("requested missing workspace {workspace_id}");
            return;
        };
        let source_workspace = self
            .workspace_for_window_id(&id)
            .or_else(|| self.workspace_for_window(&window));

        crate::diagnostics::log(format!(
            "tile:move_to_workspace id={id:?} source={source_workspace:?} target={workspace_id}"
        ));

        self.floating_windows.remove(&id);
        self.purge_workspace_tile_state(&id, true);
        self.assign_window_to_workspace(id, workspace_id);
        self.active_workspace = workspace_id;

        // Put it inside the target zone before tiling so clients that inspect
        // initial position see the same workspace they are about to join.
        let loc = Point::<i32, Logical>::from((
            workspace_rect.loc.x + self.config.snap_gap.round() as i32,
            workspace_rect.loc.y + self.config.snap_gap.round() as i32,
        ));
        self.space.map_element(window, loc, true);
        if let Some(source_workspace) = source_workspace
            && source_workspace != workspace_id
        {
            self.tile_workspace(source_workspace, false);
        }
        self.tile_workspace(workspace_id, false);
        self.stabilize_tiled_workspace_view();
    }

    pub fn reconcile_moved_tiled_window(&mut self, window: &Window) {
        if !self.in_workspace_perspective() {
            crate::diagnostics::log("tile:reconcile_moved skip=not_workspace_perspective");
            return;
        }
        if !self.is_tiling_candidate(window) {
            return;
        }
        let Some(id) = Self::window_object_id(window) else {
            return;
        };
        if self.floating_windows.contains(&id) {
            return;
        }

        let source_workspace = self.workspace_for_window_id(&id);
        let target_workspace = self
            .workspace_for_window(window)
            .or(source_workspace)
            .unwrap_or(self.active_workspace);
        crate::diagnostics::log(format!(
            "tile:reconcile_moved id={id:?} source={source_workspace:?} target={target_workspace}"
        ));

        if source_workspace == Some(target_workspace)
            && self.snap_window_to_existing_tile(window, &id)
        {
            crate::diagnostics::log(format!(
                "tile:reconcile_moved_same_workspace id={id:?} workspace={target_workspace}"
            ));
            return;
        }

        if source_workspace != Some(target_workspace) {
            self.assign_window_to_workspace(id, target_workspace);
        }

        if let Some(source_workspace) = source_workspace
            && source_workspace != target_workspace
        {
            self.tile_workspace(source_workspace, false);
        }
        self.active_workspace = target_workspace;
        self.tile_workspace(target_workspace, false);
        self.stabilize_tiled_workspace_view();
    }

    pub fn prepare_tiled_window_unmap(&mut self, id: &ObjectId) -> Option<TiledUnmapState> {
        self.floating_windows.remove(id);
        let source_workspace = self.workspace_for_window_id(id);
        let removed_rect = source_workspace.and_then(|workspace_id| {
            self.workspaces
                .get(&workspace_id)
                .and_then(|workspace| workspace.tile_rects.get(id).cloned())
        });
        crate::diagnostics::log(format!(
            "tile:prepare_unmap id={id:?} source={source_workspace:?} rect={}",
            removed_rect.is_some()
        ));
        self.purge_workspace_tile_state(id, true);
        source_workspace.map(|workspace_id| TiledUnmapState {
            workspace_id,
            removed_rect,
        })
    }

    pub fn float_window_and_retile_workspace(&mut self, window: &Window) {
        let Some(id) = Self::window_object_id(window) else {
            return;
        };
        let source_workspace = self.workspace_for_window_id(&id);
        self.floating_windows.insert(id.clone());
        self.purge_workspace_tile_state(&id, true);
        if let Some(workspace_id) = source_workspace {
            self.tile_workspace(workspace_id, false);
        }
        self.mark_all_dirty();
    }

    pub fn retile_after_window_unmap(&mut self, unmap_state: Option<TiledUnmapState>) {
        let Some(unmap_state) = unmap_state else {
            crate::diagnostics::log("tile:retile_after_unmap skip=no_workspace");
            return;
        };
        let workspace_id = unmap_state.workspace_id;

        crate::diagnostics::log(format!(
            "tile:retile_after_unmap workspace={workspace_id} perspective={} active_ws={}",
            self.in_workspace_perspective(),
            self.active_workspace
        ));
        let removed_rect = unmap_state.removed_rect;
        // Hyprland-style dwindle removal: prepare_tiled_window_unmap() already
        // promotes the sibling in the tree, so the close path must recalculate
        // from that tree instead of merging stale rectangles by hand.
        self.tile_workspace(workspace_id, false);
        if let Some(removed_rect) = removed_rect {
            self.focus_nearest_tiled_after_unmap(workspace_id, removed_rect);
        }
        if self.in_workspace_perspective() && workspace_id == self.active_workspace {
            self.stabilize_tiled_workspace_view();
        }
    }

    pub fn tile_current_workspace_windows(&mut self) {
        if let Some(workspace_id) = self.workspace_at_pointer() {
            self.active_workspace = workspace_id;
        } else {
            self.sync_active_workspace_from_pointer();
        }
        let workspace_id = self.active_workspace;
        crate::diagnostics::log(format!(
            "tile:current_workspace_begin workspace={workspace_id} perspective={} zoom={:.3}",
            self.in_workspace_perspective(),
            self.zoom()
        ));

        let ids: Vec<ObjectId> = self
            .space
            .elements()
            .filter(|window| !window.is_widget())
            .filter(|window| self.workspace_for_window(window) == Some(workspace_id))
            .filter_map(Self::window_object_id)
            .collect();

        if ids.is_empty() {
            crate::diagnostics::log(format!(
                "tile:current_workspace_skip workspace={workspace_id} reason=no_windows"
            ));
            return;
        }
        crate::diagnostics::log(format!(
            "tile:current_workspace_collect workspace={workspace_id} ids={}",
            ids.len()
        ));

        for id in &ids {
            self.floating_windows.remove(id);
        }

        for workspace in self.workspaces.values_mut() {
            for id in &ids {
                workspace.windows.remove(id);
                workspace.tile_rects.remove(id);
            }
        }

        let Some(workspace) = self.workspaces.get_mut(&workspace_id) else {
            return;
        };
        for id in ids {
            workspace.windows.insert(id);
        }

        self.tile_workspace(workspace_id, false);
        if self.in_workspace_perspective() {
            self.stabilize_tiled_workspace_view();
        }
    }

    pub fn toggle_dwindle_split_current_workspace(&mut self) {
        if let Some(workspace_id) = self.workspace_at_pointer() {
            self.active_workspace = workspace_id;
        } else {
            self.sync_active_workspace_from_pointer();
        }

        let workspace_id = self.active_workspace;
        let target_id = self
            .hovered_or_focused_window()
            .and_then(|window| Self::window_object_id(&window));
        let Some(workspace) = self.workspaces.get_mut(&workspace_id) else {
            return;
        };

        let leaf_id = target_id
            .as_ref()
            .and_then(|id| find_dwindle_leaf(workspace, id))
            .or_else(|| workspace.dwindle_root);
        let Some(parent_id) = leaf_id.and_then(|leaf| {
            workspace
                .dwindle_nodes
                .get(&leaf)
                .and_then(|node| node.parent)
        }) else {
            crate::diagnostics::log(format!(
                "tile:toggle_dwindle_split_skip workspace={workspace_id} reason=no_parent"
            ));
            return;
        };

        let split = if let Some(parent) = workspace.dwindle_nodes.get_mut(&parent_id) {
            parent.split = parent.split.toggled();
            parent.split
        } else {
            return;
        };
        let split_name = match split {
            DwindleSplit::Vertical => "vertical",
            DwindleSplit::Horizontal => "horizontal",
        };
        crate::diagnostics::log(format!(
            "tile:toggle_dwindle_split workspace={workspace_id} parent={parent_id} split={split_name}"
        ));

        self.tile_workspace(workspace_id, false);
        if self.in_workspace_perspective() {
            self.stabilize_tiled_workspace_view();
        }
    }

    fn workspace_tile_area(&self, workspace_id: WorkspaceId) -> Rectangle<i32, Logical> {
        let workspace = self
            .workspaces
            .get(&workspace_id)
            .or_else(|| self.workspaces.get(&1))
            .map(|workspace| workspace.rect)
            .unwrap_or_else(|| Rectangle::new(Point::from((0, 0)), Size::from((1920, 1080))));
        let gap = self.config.snap_gap.max(0.0).round() as i32;
        Rectangle::new(
            Point::from((workspace.loc.x + gap, workspace.loc.y + gap)),
            Size::from((
                (workspace.size.w - gap * 2).max(80),
                (workspace.size.h - gap * 2).max(80),
            )),
        )
    }

    pub fn prepare_new_tiled_window_spawn(
        &mut self,
        window: &Window,
    ) -> Option<(WorkspaceId, Point<i32, Logical>)> {
        if !self.is_tiling_candidate(window) {
            crate::diagnostics::log(format!(
                "tile:new_spawn_skip candidate=false app={:?} title={:?} parent={} modal={} widget={} x11={}",
                window.app_id_or_class(),
                window.window_title(),
                window.parent_surface().is_some(),
                window.is_modal(),
                window.is_widget(),
                window.x11_surface().is_some()
            ));
            return None;
        }
        let id = Self::window_object_id(window)?;
        let anchor_workspace = self
            .pending_tile_anchors
            .get(&id)
            .and_then(|anchor_id| self.workspace_for_window_id(anchor_id));
        let pointer_workspace = self.workspace_at_pointer();
        let workspace_id = anchor_workspace.or(pointer_workspace).or_else(|| {
            self.in_workspace_perspective()
                .then_some(self.active_workspace)
        })?;
        let tile_area = self.workspace_tile_area(workspace_id);

        self.assign_window_to_workspace(id.clone(), workspace_id);
        self.active_workspace = workspace_id;
        crate::diagnostics::log(format!(
            "tile:new_spawn id={id:?} workspace={workspace_id} loc=({}, {}) anchor_workspace={anchor_workspace:?}",
            tile_area.loc.x, tile_area.loc.y
        ));
        Some((workspace_id, tile_area.loc))
    }

    pub fn tile_workspace_for_new_spawn(&mut self, workspace_id: WorkspaceId) {
        self.tile_workspace(workspace_id, false);
    }

    pub fn stabilize_tiled_workspace_view(&mut self) {
        self.set_camera_target(None);
        self.set_zoom_target(None);
        self.set_zoom_animation_center(None);
        self.set_overview_return(None);
        if (self.zoom() - 1.0).abs() > 1e-9 {
            self.set_zoom(1.0);
        }
        let workspace = self.active_workspace_rect();
        let vc = self.usable_center_screen();
        let zoom = self.zoom();
        let center = Point::<f64, Logical>::from((
            workspace.loc.x as f64 + workspace.size.w as f64 / 2.0,
            workspace.loc.y as f64 + workspace.size.h as f64 / 2.0,
        ));
        let camera = Point::from((center.x - vc.x / zoom, center.y - vc.y / zoom));
        self.set_camera(camera);
        self.update_output_from_camera();
    }

    pub fn tile_windows(&mut self) {
        if let Some(workspace_id) = self.workspace_at_pointer() {
            self.active_workspace = workspace_id;
        } else if !self.in_workspace_perspective() {
            crate::diagnostics::log(format!(
                "tile:workspace_skip reason=no_pointer_workspace zoom={:.3}",
                self.zoom()
            ));
            return;
        } else {
            self.sync_active_workspace_from_pointer();
        }
        self.tile_workspace(self.active_workspace, true);
    }

    fn tile_workspace(&mut self, workspace_id: WorkspaceId, assign_unassigned: bool) {
        let started = Instant::now();
        self.floating_windows.retain(|id| {
            self.space
                .elements()
                .any(|window| Self::window_object_id(window).as_ref() == Some(id))
        });

        let windows: Vec<Window> = self
            .space
            .elements()
            .filter(|window| self.is_tiling_candidate(window))
            .cloned()
            .collect();
        let count = windows.len();
        crate::diagnostics::log(format!(
            "tile:workspace_begin workspace={workspace_id} assign_unassigned={assign_unassigned} candidates={count} floating={} active_ws={} perspective={}",
            self.floating_windows.len(),
            self.active_workspace,
            self.in_workspace_perspective()
        ));
        if count == 0 {
            if let Some(workspace) = self.workspaces.get_mut(&workspace_id) {
                workspace.windows.clear();
                workspace.tile_rects.clear();
            }
            crate::diagnostics::log(format!(
                "tile:workspace_end workspace={workspace_id} reason=no_candidates elapsed_ms={}",
                started.elapsed().as_millis()
            ));
            return;
        }

        let all_window_ids: Vec<ObjectId> =
            windows.iter().filter_map(Self::window_object_id).collect();
        crate::diagnostics::log(format!(
            "tile:workspace_phase workspace={workspace_id} phase=ids ids={}",
            all_window_ids.len()
        ));
        for workspace in self.workspaces.values_mut() {
            workspace
                .windows
                .retain(|id| all_window_ids.iter().any(|window_id| window_id == id));
            workspace
                .tile_rects
                .retain(|id, _| workspace.windows.contains(id));
        }
        crate::diagnostics::log(format!(
            "tile:workspace_phase workspace={workspace_id} phase=purged"
        ));

        let assigned_ids: Vec<ObjectId> = self
            .workspaces
            .values()
            .flat_map(|workspace| workspace.windows.iter().cloned())
            .collect();
        crate::diagnostics::log(format!(
            "tile:workspace_phase workspace={workspace_id} phase=assigned assigned={}",
            assigned_ids.len()
        ));
        if assign_unassigned && let Some(workspace) = self.workspaces.get_mut(&workspace_id) {
            for id in &all_window_ids {
                if !assigned_ids.iter().any(|assigned| assigned == id) {
                    workspace.windows.insert(id.clone());
                }
            }
        }

        let active_window_ids_unordered: Vec<ObjectId> = self
            .workspaces
            .get(&workspace_id)
            .map(|workspace| workspace.windows.iter().cloned().collect())
            .unwrap_or_default();
        crate::diagnostics::log(format!(
            "tile:workspace_phase workspace={workspace_id} phase=active_ids active_ids={}",
            active_window_ids_unordered.len()
        ));
        let windows: Vec<Window> = windows
            .into_iter()
            .filter(|window| {
                Self::window_object_id(window).is_some_and(|id| {
                    active_window_ids_unordered
                        .iter()
                        .any(|active_id| active_id == &id)
                })
            })
            .collect();
        if windows.is_empty() {
            if let Some(workspace) = self.workspaces.get_mut(&workspace_id) {
                workspace.tile_rects.clear();
            }
            crate::diagnostics::log(format!(
                "tile:workspace_end workspace={workspace_id} reason=no_workspace_windows elapsed_ms={}",
                started.elapsed().as_millis()
            ));
            return;
        }

        let active_window_ids: Vec<ObjectId> =
            windows.iter().filter_map(Self::window_object_id).collect();
        crate::diagnostics::log(format!(
            "tile:workspace_phase workspace={workspace_id} phase=filtered filtered={}",
            active_window_ids.len()
        ));
        let full_area = self.workspace_tile_area(workspace_id);
        let gap = self.config.snap_gap.max(0.0).round() as i32;
        let pointer = self
            .seat
            .get_pointer()
            .map(|pointer| pointer.current_location().to_i32_round::<i32>());

        let active_ids = active_window_ids;
        let pending_anchors: std::collections::HashMap<ObjectId, Option<ObjectId>> = active_ids
            .iter()
            .map(|id| (id.clone(), self.pending_tile_anchors.remove(id)))
            .collect();

        let mut next_tile_rects = std::collections::HashMap::new();
        if let Some(workspace) = self.workspaces.get_mut(&workspace_id) {
            let existing_leaf_ids: Vec<ObjectId> = workspace
                .dwindle_nodes
                .values()
                .filter_map(|node| node.window.clone())
                .filter(|id| !active_ids.iter().any(|active_id| active_id == id))
                .collect();
            for id in existing_leaf_ids {
                remove_dwindle_window(workspace, &id);
            }

            workspace.tile_rects.clear();
            if let Some(root) = workspace.dwindle_root {
                let mut calculated = std::collections::HashMap::new();
                calculate_dwindle_rects(workspace, root, full_area, gap, &mut calculated);
                workspace.tile_rects = calculated;
            }

            for new_id in &active_ids {
                if find_dwindle_leaf(workspace, new_id).is_some() {
                    continue;
                }
                crate::diagnostics::log(format!(
                    "tile:workspace_phase workspace={workspace_id} phase=tree_insert id={new_id:?}"
                ));
                let anchor = pending_anchors.get(new_id).and_then(|id| id.clone());
                insert_dwindle_window(workspace, new_id.clone(), anchor, pointer, gap);
                if let Some(root) = workspace.dwindle_root {
                    let mut calculated = std::collections::HashMap::new();
                    calculate_dwindle_rects(workspace, root, full_area, gap, &mut calculated);
                    workspace.tile_rects = calculated;
                }
            }

            next_tile_rects = workspace.tile_rects.clone();
        }
        crate::diagnostics::log(format!(
            "tile:workspace_phase workspace={workspace_id} phase=rects rects={}",
            next_tile_rects.len()
        ));

        let mut configured = 0usize;
        let mut remapped = 0usize;
        for window in windows.iter() {
            let Some(id) = Self::window_object_id(window) else {
                continue;
            };
            let Some(rect) = next_tile_rects.get(&id).cloned() else {
                continue;
            };
            let loc = rect.loc;
            let size = rect.size;
            let current_loc = self.space.element_location(window);
            let current_size = window.geometry().size;
            let already_tiled = current_loc == Some(loc) && current_size == size;
            crate::diagnostics::log(format!(
                "tile:rect workspace={workspace_id} id={id:?} app={:?} title={:?} target=({}, {}) {}x{} current_loc={:?} current_size={}x{} already={already_tiled}",
                window.app_id_or_class(),
                window.window_title(),
                loc.x,
                loc.y,
                size.w,
                size.h,
                current_loc,
                current_size.w,
                current_size.h
            ));

            if let Some(toplevel) = window.toplevel() {
                crate::handlers::set_tiled_states(&toplevel);
                if !already_tiled {
                    toplevel.with_pending_state(|state| {
                        state.size = Some(size);
                    });
                    toplevel.send_configure();
                    configured += 1;
                }
            }
            if !already_tiled {
                self.space.map_element(window.clone(), loc, false);
                remapped += 1;
            }
        }
        crate::diagnostics::log(format!(
            "tile:workspace_phase workspace={workspace_id} phase=configured configured={configured} remapped={remapped}"
        ));
        self.sync_pointer_focus_under_cursor();
        crate::diagnostics::log(format!(
            "tile:workspace_phase workspace={workspace_id} phase=focus_synced"
        ));
        self.mark_all_dirty();
        crate::diagnostics::log(format!(
            "tile:workspace_end workspace={workspace_id} windows={} configured={configured} remapped={remapped} elapsed_ms={}",
            windows.len(),
            started.elapsed().as_millis()
        ));
    }
}
