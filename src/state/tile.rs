use driftwm::window_ext::WindowExt;
use smithay::desktop::Window;
use smithay::reexports::wayland_server::{Resource, backend::ObjectId};
use smithay::utils::{Logical, Point, Rectangle, Size};
use smithay::wayland::seat::WaylandFocus;

use super::{DriftWm, FocusTarget, WorkspaceId};

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

    fn workspace_for_window_id(&self, id: &ObjectId) -> Option<WorkspaceId> {
        self.workspaces
            .iter()
            .find_map(|(&workspace_id, workspace)| {
                workspace.windows.contains(id).then_some(workspace_id)
            })
    }

    fn is_tiling_candidate(&self, window: &Window) -> bool {
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
        self.sync_active_workspace_from_pointer();
        self.tile_workspace(self.active_workspace, true);
    }

    fn tile_workspace(&mut self, workspace_id: WorkspaceId, assign_unassigned: bool) {
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
        if count == 0 {
            if let Some(workspace) = self.workspaces.get_mut(&workspace_id) {
                workspace.windows.clear();
                workspace.tile_rects.clear();
            }
            return;
        }

        let all_window_ids: Vec<ObjectId> =
            windows.iter().filter_map(Self::window_object_id).collect();
        for workspace in self.workspaces.values_mut() {
            workspace
                .windows
                .retain(|id| all_window_ids.iter().any(|window_id| window_id == id));
            workspace
                .tile_rects
                .retain(|id, _| workspace.windows.contains(id));
        }

        let assigned_ids: Vec<ObjectId> = self
            .workspaces
            .values()
            .flat_map(|workspace| workspace.windows.iter().cloned())
            .collect();
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
            return;
        }

        let active_window_ids: Vec<ObjectId> =
            windows.iter().filter_map(Self::window_object_id).collect();
        let full_area = self.workspace_tile_area(workspace_id);
        let gap = self.config.snap_gap.max(0.0).round() as i32;
        // Rebuild the active workspace tree from current tiled membership.
        // This mirrors Hyprland's remove/reinsert/recalculate behavior and
        // guarantees the remaining windows expand after a float toggle.
        let mut next_tile_rects = std::collections::HashMap::new();
        let new_ids = active_window_ids;

        for new_id in new_ids {
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

        for window in windows.iter() {
            let Some(id) = Self::window_object_id(window) else {
                continue;
            };
            let Some(rect) = next_tile_rects.get(&id).cloned() else {
                continue;
            };
            let loc = rect.loc;
            let size = rect.size;

            if let Some(toplevel) = window.toplevel() {
                crate::handlers::set_tiled_states(&toplevel);
                toplevel.with_pending_state(|state| {
                    state.size = Some(size);
                });
                toplevel.send_configure();
            }
            self.space.map_element(window.clone(), loc, false);
        }
        self.sync_pointer_focus_under_cursor();
        self.mark_all_dirty();
    }
}
