use driftwm::window_ext::WindowExt;
use smithay::desktop::Window;
use smithay::reexports::wayland_server::{Resource, backend::ObjectId};
use smithay::utils::{Logical, Point, Size};
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
            self.floating_windows.insert(id);
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

        let usable = self.get_usable_area();
        let camera = self.camera().to_i32_round::<i32>();
        let gap = self.config.snap_gap.max(0.0).round() as i32;

        let area_x = camera.x + usable.loc.x;
        let area_y = camera.y + usable.loc.y;
        let area_w = usable.size.w;
        let area_h = usable.size.h;

        let cols = (count as f64).sqrt().ceil() as i32;
        let rows = ((count as i32 + cols - 1) / cols).max(1);
        let tile_w = ((area_w - gap * (cols + 1)) / cols).max(80);
        let tile_h = ((area_h - gap * (rows + 1)) / rows).max(80);

        for (idx, window) in windows.iter().enumerate() {
            let idx = idx as i32;
            let col = idx % cols;
            let row = idx / cols;
            let loc = Point::<i32, Logical>::from((
                area_x + gap + col * (tile_w + gap),
                area_y + gap + row * (tile_h + gap),
            ));
            let size = Size::<i32, Logical>::from((tile_w, tile_h));

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
