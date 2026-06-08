use driftwm::window_ext::WindowExt;
use smithay::desktop::Window;
use smithay::reexports::wayland_server::{Resource, backend::ObjectId};
use smithay::utils::{Logical, Point, Rectangle, Size};
use smithay::wayland::seat::WaylandFocus;
use std::time::Instant;

use super::{DriftWm, FocusTarget, WorkspaceId};

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
            }
        }
    }

    fn apply_workspace_tile_rects(&mut self, workspace_id: WorkspaceId) {
        let Some(workspace) = self.workspaces.get(&workspace_id) else {
            return;
        };
        let tile_rects = workspace.tile_rects.clone();
        if tile_rects.is_empty() {
            return;
        }

        let windows: Vec<Window> = self
            .space
            .elements()
            .filter(|window| self.is_tiling_candidate(window))
            .filter(|window| {
                Self::window_object_id(window)
                    .as_ref()
                    .is_some_and(|id| tile_rects.contains_key(id))
            })
            .cloned()
            .collect();

        let mut configured = 0usize;
        let mut remapped = 0usize;
        for window in &windows {
            let Some(id) = Self::window_object_id(window) else {
                continue;
            };
            let Some(rect) = tile_rects.get(&id).cloned() else {
                continue;
            };
            let already_tiled = self.space.element_location(window) == Some(rect.loc)
                && window.geometry().size == rect.size;

            if let Some(toplevel) = window.toplevel() {
                crate::handlers::set_tiled_states(&toplevel);
                if !already_tiled {
                    toplevel.with_pending_state(|state| {
                        state.size = Some(rect.size);
                    });
                    toplevel.send_configure();
                    configured += 1;
                }
            }
            if !already_tiled {
                self.space.map_element(window.clone(), rect.loc, false);
                remapped += 1;
            }
        }
        self.sync_pointer_focus_under_cursor();
        self.mark_all_dirty();
        crate::diagnostics::log(format!(
            "tile:apply_existing workspace={workspace_id} windows={} configured={configured} remapped={remapped}",
            windows.len()
        ));
    }

    fn merge_removed_tile_into_sibling(
        &mut self,
        workspace_id: WorkspaceId,
        removed_rect: Rectangle<i32, Logical>,
    ) -> bool {
        let gap = self.config.snap_gap.max(0.0).round() as i32;
        let Some(workspace) = self.workspaces.get_mut(&workspace_id) else {
            return false;
        };

        let mut best: Option<(ObjectId, Rectangle<i32, Logical>, i32)> = None;
        for (id, rect) in &workspace.tile_rects {
            let horizontal_neighbor = rect.loc.y == removed_rect.loc.y
                && rect.size.h == removed_rect.size.h
                && (rect.loc.x + rect.size.w + gap == removed_rect.loc.x
                    || removed_rect.loc.x + removed_rect.size.w + gap == rect.loc.x);
            let vertical_neighbor = rect.loc.x == removed_rect.loc.x
                && rect.size.w == removed_rect.size.w
                && (rect.loc.y + rect.size.h + gap == removed_rect.loc.y
                    || removed_rect.loc.y + removed_rect.size.h + gap == rect.loc.y);

            if !horizontal_neighbor && !vertical_neighbor {
                continue;
            }

            let min_x = rect.loc.x.min(removed_rect.loc.x);
            let min_y = rect.loc.y.min(removed_rect.loc.y);
            let max_x = (rect.loc.x + rect.size.w).max(removed_rect.loc.x + removed_rect.size.w);
            let max_y = (rect.loc.y + rect.size.h).max(removed_rect.loc.y + removed_rect.size.h);
            let merged = Rectangle::new(
                Point::from((min_x, min_y)),
                Size::from((max_x - min_x, max_y - min_y)),
            );
            let area = rect.size.w.saturating_mul(rect.size.h);
            if best
                .as_ref()
                .is_none_or(|(_, _, best_area)| area > *best_area)
            {
                best = Some((id.clone(), merged, area));
            }
        }

        let Some((sibling_id, merged, _)) = best else {
            return false;
        };

        workspace.tile_rects.insert(sibling_id.clone(), merged);
        self.tile_rects.insert(sibling_id.clone(), merged);
        crate::diagnostics::log(format!(
            "tile:merge_removed workspace={workspace_id} sibling={sibling_id:?} merged=({}, {}) {}x{}",
            merged.loc.x, merged.loc.y, merged.size.w, merged.size.h
        ));
        true
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
            crate::handlers::set_tiled_states(&toplevel);
            toplevel.with_pending_state(|state| {
                state.size = Some(rect.size);
            });
            toplevel.send_configure();
        }
        self.space.map_element(window.clone(), rect.loc, false);
        self.sync_pointer_focus_under_cursor();
        self.mark_all_dirty();
        true
    }

    fn is_tiling_candidate(&self, window: &Window) -> bool {
        if window.x11_surface().is_some() {
            return false;
        }
        if window.is_widget() || window.parent_surface().is_some() || window.is_modal() {
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
        if let Some(removed_rect) = unmap_state.removed_rect
            && self.merge_removed_tile_into_sibling(workspace_id, removed_rect)
        {
            self.apply_workspace_tile_rects(workspace_id);
            if self.in_workspace_perspective() && workspace_id == self.active_workspace {
                self.stabilize_tiled_workspace_view();
            }
            return;
        }

        self.tile_workspace(workspace_id, false);
        if self.in_workspace_perspective() && workspace_id == self.active_workspace {
            self.stabilize_tiled_workspace_view();
        }
    }

    pub fn tile_current_workspace_windows(&mut self) {
        self.sync_active_workspace_from_pointer();
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

    fn split_rect(
        rect: Rectangle<i32, Logical>,
        gap: i32,
    ) -> (Rectangle<i32, Logical>, Rectangle<i32, Logical>) {
        if rect.size.w >= rect.size.h {
            let first_w = ((rect.size.w - gap) / 2).max(80);
            let second_w = (rect.size.w - first_w - gap).max(80);
            (
                Rectangle::new(rect.loc, Size::from((first_w, rect.size.h))),
                Rectangle::new(
                    Point::from((rect.loc.x + first_w + gap, rect.loc.y)),
                    Size::from((second_w, rect.size.h)),
                ),
            )
        } else {
            let first_h = ((rect.size.h - gap) / 2).max(80);
            let second_h = (rect.size.h - first_h - gap).max(80);
            (
                Rectangle::new(rect.loc, Size::from((rect.size.w, first_h))),
                Rectangle::new(
                    Point::from((rect.loc.x, rect.loc.y + first_h + gap)),
                    Size::from((rect.size.w, second_h)),
                ),
            )
        }
    }

    fn largest_tiled_window(
        &self,
        windows: &[Window],
        tile_rects: &std::collections::HashMap<ObjectId, Rectangle<i32, Logical>>,
    ) -> Option<ObjectId> {
        windows
            .iter()
            .filter_map(|window| {
                let id = Self::window_object_id(window)?;
                let rect = tile_rects.get(&id)?;
                Some((id, rect.size.w.saturating_mul(rect.size.h)))
            })
            .max_by_key(|(_, area)| *area)
            .map(|(id, _)| id)
    }

    fn pointer_tiled_window(
        &self,
        windows: &[Window],
        tile_rects: &std::collections::HashMap<ObjectId, Rectangle<i32, Logical>>,
    ) -> Option<ObjectId> {
        let pointer = self.seat.get_pointer()?;
        let pos = pointer.current_location().to_i32_round::<i32>();
        windows.iter().find_map(|window| {
            let id = Self::window_object_id(window)?;
            let rect = tile_rects.get(&id)?;
            rect.contains(pos).then_some(id)
        })
    }

    fn focused_tiled_window(
        &self,
        windows: &[Window],
        tile_rects: &std::collections::HashMap<ObjectId, Rectangle<i32, Logical>>,
    ) -> Option<ObjectId> {
        let focused = self.focused_window()?;
        let focused_id = Self::window_object_id(&focused)?;
        windows
            .iter()
            .filter_map(Self::window_object_id)
            .any(|id| id == focused_id && tile_rects.contains_key(&id))
            .then_some(focused_id)
    }

    pub fn tile_windows(&mut self) {
        if !self.in_workspace_perspective() {
            return;
        }
        self.sync_active_workspace_from_pointer();
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
        // Rebuild the active workspace tree from current tiled membership.
        // This mirrors Hyprland's remove/reinsert/recalculate behavior and
        // guarantees the remaining windows expand after a float toggle.
        let mut next_tile_rects = std::collections::HashMap::new();
        let new_ids = active_window_ids;

        for new_id in new_ids {
            crate::diagnostics::log(format!(
                "tile:workspace_phase workspace={workspace_id} phase=place id={new_id:?} placed={}",
                next_tile_rects.len()
            ));
            if next_tile_rects.is_empty() {
                next_tile_rects.insert(new_id, full_area);
                continue;
            }

            let anchor_id = self
                .pending_tile_anchors
                .remove(&new_id)
                .filter(|id| next_tile_rects.contains_key(id))
                .or_else(|| self.pointer_tiled_window(&windows, &next_tile_rects))
                .or_else(|| self.focused_tiled_window(&windows, &next_tile_rects))
                .or_else(|| self.largest_tiled_window(&windows, &next_tile_rects));

            let Some(anchor_id) = anchor_id else {
                next_tile_rects.insert(new_id, full_area);
                continue;
            };
            let Some(anchor_rect) = next_tile_rects.get(&anchor_id).cloned() else {
                next_tile_rects.insert(new_id, full_area);
                continue;
            };

            let (anchor_new_rect, new_rect) = Self::split_rect(anchor_rect, gap);
            next_tile_rects.insert(anchor_id, anchor_new_rect);
            next_tile_rects.insert(new_id, new_rect);
        }

        if let Some(workspace) = self.workspaces.get_mut(&workspace_id) {
            workspace.tile_rects = next_tile_rects.clone();
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
