use driftwm::window_ext::WindowExt;
use smithay::desktop::Window;
use smithay::reexports::wayland_server::{Resource, backend::ObjectId};
use smithay::utils::{Logical, Point, Rectangle, Size};
use smithay::wayland::seat::WaylandFocus;

use super::{DriftWm, FocusTarget};

impl DriftWm {
    fn window_object_id(window: &Window) -> Option<ObjectId> {
        window.wl_surface().map(|surface| surface.id())
    }

    fn is_tiling_candidate(&self, window: &Window) -> bool {
        if window.is_widget() || window.parent_surface().is_some() || window.is_modal() {
            return false;
        }
        if self.fullscreen.values().any(|fs| fs.window == *window) {
            return false;
        }
        Self::window_object_id(window)
            .is_some_and(|id| !self.floating_windows.contains(&id))
    }

    pub fn toggle_floating_window(&mut self) {
        let Some(window) = self.focused_window().filter(|w| !w.is_widget()) else {
            return;
        };
        let Some(id) = Self::window_object_id(&window) else {
            return;
        };

        if self.floating_windows.remove(&id) {
            self.tile_windows();
        } else {
            self.floating_windows.insert(id.clone());
            self.tile_rects.remove(&id);
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
                self.seat
                    .get_keyboard()
                    .unwrap()
                    .set_focus(self, Some(FocusTarget(surface.into_owned())), serial);
            }
        }
    }

    fn current_tile_area(&self) -> Rectangle<i32, Logical> {
        let usable = self.get_usable_area();
        let camera = self.camera().to_i32_round::<i32>();
        let gap = self.config.snap_gap.max(0.0).round() as i32;
        Rectangle::new(
            Point::from((camera.x + usable.loc.x + gap, camera.y + usable.loc.y + gap)),
            Size::from((
                (usable.size.w - gap * 2).max(80),
                (usable.size.h - gap * 2).max(80),
            )),
        )
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

    fn largest_tiled_window(&self, windows: &[Window]) -> Option<ObjectId> {
        windows
            .iter()
            .filter_map(|window| {
                let id = Self::window_object_id(window)?;
                let rect = self.tile_rects.get(&id)?;
                Some((id, rect.size.w.saturating_mul(rect.size.h)))
            })
            .max_by_key(|(_, area)| *area)
            .map(|(id, _)| id)
    }

    fn pointer_tiled_window(&self, windows: &[Window]) -> Option<ObjectId> {
        let pointer = self.seat.get_pointer()?;
        let pos = pointer.current_location().to_i32_round::<i32>();
        windows.iter().find_map(|window| {
            let id = Self::window_object_id(window)?;
            let rect = self.tile_rects.get(&id)?;
            rect.contains(pos).then_some(id)
        })
    }

    fn focused_tiled_window(&self, windows: &[Window]) -> Option<ObjectId> {
        let focused = self.focused_window()?;
        let focused_id = Self::window_object_id(&focused)?;
        windows
            .iter()
            .filter_map(Self::window_object_id)
            .any(|id| id == focused_id)
            .then_some(focused_id)
    }

    pub fn tile_windows(&mut self) {
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
            return;
        }

        let window_ids: Vec<ObjectId> = windows
            .iter()
            .filter_map(Self::window_object_id)
            .collect();
        self.tile_rects
            .retain(|id, _| window_ids.iter().any(|window_id| window_id == id));

        let full_area = self.current_tile_area();
        let gap = self.config.snap_gap.max(0.0).round() as i32;
        let new_ids: Vec<ObjectId> = window_ids
            .iter()
            .filter(|id| !self.tile_rects.contains_key(id))
            .cloned()
            .collect();

        for new_id in new_ids {
            if self.tile_rects.is_empty() {
                self.tile_rects.insert(new_id, full_area);
                continue;
            }

            let anchor_id = self
                .pending_tile_anchors
                .remove(&new_id)
                .filter(|id| self.tile_rects.contains_key(id))
                .or_else(|| self.pointer_tiled_window(&windows))
                .or_else(|| self.focused_tiled_window(&windows))
                .or_else(|| self.largest_tiled_window(&windows));

            let Some(anchor_id) = anchor_id else {
                self.tile_rects.insert(new_id, full_area);
                continue;
            };
            let Some(anchor_rect) = self.tile_rects.get(&anchor_id).cloned() else {
                self.tile_rects.insert(new_id, full_area);
                continue;
            };

            let (anchor_new_rect, new_rect) = Self::split_rect(anchor_rect, gap);
            self.tile_rects.insert(anchor_id, anchor_new_rect);
            self.tile_rects.insert(new_id, new_rect);
        }

        for window in windows.iter() {
            let Some(id) = Self::window_object_id(window) else {
                continue;
            };
            let Some(rect) = self.tile_rects.get(&id).cloned() else {
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
        self.mark_all_dirty();
    }
}
