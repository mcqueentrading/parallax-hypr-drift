use smithay::{
    desktop::Window,
    reexports::wayland_server::{Resource, backend::ObjectId},
    utils::{Logical, Point, Rectangle},
    wayland::seat::WaylandFocus,
    wayland::xwayland_shell::{XWaylandShellHandler, XWaylandShellState},
    xwayland::{
        X11Surface, X11Wm, XwmHandler,
        xwm::{Reorder, ResizeEdge as X11ResizeEdge, XwmId},
    },
};

use crate::state::DriftWm;

impl XWaylandShellHandler for DriftWm {
    fn xwayland_shell_state(&mut self) -> &mut XWaylandShellState {
        &mut self.xwayland_shell_state
    }
}

impl XwmHandler for DriftWm {
    fn xwm_state(&mut self, _xwm: XwmId) -> &mut X11Wm {
        self.xwm.as_mut().expect("XWM event without active XWM")
    }

    fn new_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn new_override_redirect_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn map_window_request(&mut self, _xwm: XwmId, surface: X11Surface) {
        if let Err(err) = surface.set_mapped(true) {
            tracing::warn!("failed to mark X11 window mapped: {err}");
        }

        let window = Window::new_x11_window(surface.clone());
        let pos = self
            .seat
            .get_pointer()
            .map(|pointer| pointer.current_location())
            .map(|pos| Point::from((pos.x.round() as i32, pos.y.round() as i32)))
            .unwrap_or_else(|| {
                let camera = self.camera();
                Point::from((camera.x.round() as i32, camera.y.round() as i32))
            });

        self.space.map_element(window.clone(), pos, true);
        self.space.raise_element(&window, true);
        if let Some(rect) = self.space.element_bbox(&window)
            && let Err(err) = surface.configure(Some(rect))
        {
            tracing::warn!("failed to configure new X11 window: {err}");
        }
        if self.in_workspace_perspective() {
            self.tile_windows();
            self.stabilize_tiled_workspace_view();
        } else {
            self.mark_all_dirty();
        }
        self.sync_pointer_focus_under_cursor();
    }

    fn mapped_override_redirect_window(&mut self, _xwm: XwmId, surface: X11Surface) {
        let location = surface.geometry().loc;
        let window = Window::new_x11_window(surface);
        self.space.map_element(window, location, true);
        self.mark_all_dirty();
    }

    fn unmapped_window(&mut self, _xwm: XwmId, surface: X11Surface) {
        if let Some(window) = self.x11_window_for_surface(&surface) {
            let cleanup = Self::x11_window_id(&window)
                .as_ref()
                .and_then(|id| self.prepare_tiled_window_unmap(id));
            self.space.unmap_elem(&window);
            self.retile_after_window_unmap(cleanup);
        }
        if !surface.is_override_redirect()
            && let Err(err) = surface.set_mapped(false)
        {
            tracing::warn!("failed to mark X11 window unmapped: {err}");
        }
        self.mark_all_dirty();
    }

    fn destroyed_window(&mut self, _xwm: XwmId, surface: X11Surface) {
        if let Some(window) = self.x11_window_for_surface(&surface) {
            let cleanup = Self::x11_window_id(&window)
                .as_ref()
                .and_then(|id| self.prepare_tiled_window_unmap(id));
            self.space.unmap_elem(&window);
            self.retile_after_window_unmap(cleanup);
        }
        self.mark_all_dirty();
    }

    fn configure_request(
        &mut self,
        _xwm: XwmId,
        surface: X11Surface,
        x: Option<i32>,
        y: Option<i32>,
        w: Option<u32>,
        h: Option<u32>,
        _reorder: Option<Reorder>,
    ) {
        let mut geo = surface.geometry();
        if let Some(x) = x {
            geo.loc.x = x;
        }
        if let Some(y) = y {
            geo.loc.y = y;
        }
        if let Some(w) = w {
            geo.size.w = w as i32;
        }
        if let Some(h) = h {
            geo.size.h = h as i32;
        }
        if let Err(err) = surface.configure(geo) {
            tracing::warn!("failed X11 configure request: {err}");
        }
    }

    fn configure_notify(
        &mut self,
        _xwm: XwmId,
        surface: X11Surface,
        geometry: Rectangle<i32, Logical>,
        _above: Option<u32>,
    ) {
        if let Some(window) = self.x11_window_for_surface(&surface) {
            self.space.map_element(window, geometry.loc, false);
            self.mark_all_dirty();
        }
    }

    fn resize_request(
        &mut self,
        _xwm: XwmId,
        _surface: X11Surface,
        _button: u32,
        _edges: X11ResizeEdge,
    ) {
        // Forced tiling owns geometry in this first native-XWayland pass.
    }

    fn move_request(&mut self, _xwm: XwmId, surface: X11Surface, _button: u32) {
        if let Some(window) = self.x11_window_for_surface(&surface) {
            self.space.raise_element(&window, true);
            self.sync_pointer_focus_under_cursor();
            self.mark_all_dirty();
        }
    }

    fn disconnected(&mut self, _xwm: XwmId) {
        self.xdisplay = None;
        self.xwm = None;
    }
}

impl DriftWm {
    fn x11_window_for_surface(&self, surface: &X11Surface) -> Option<Window> {
        self.space
            .elements()
            .find(|window| window.x11_surface().is_some_and(|x11| x11 == surface))
            .cloned()
    }

    fn x11_window_id(window: &Window) -> Option<ObjectId> {
        window.wl_surface().map(|surface| surface.id())
    }
}
