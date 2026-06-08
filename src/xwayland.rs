//! Native Smithay XWayland integration.
//!
//! Hyprland owns XWayland directly and exports DISPLAY from the compositor
//! session. This module follows that model using Smithay's XWayland/XWM path
//! instead of the previous xwayland-satellite bridge.

use std::process::{Command, Stdio};

use smithay::xwayland::{X11Wm, XWayland, XWaylandEvent};

use crate::state::DriftWm;

pub fn setup(state: &mut DriftWm) {
    if state.xwm.is_some() || !state.config.xwayland_enabled {
        crate::diagnostics::log(format!(
            "xwayland:skip active={} enabled={}",
            state.xwm.is_some(),
            state.config.xwayland_enabled
        ));
        return;
    }

    crate::diagnostics::log("xwayland:spawn_begin");
    let (xwayland, client) = match XWayland::spawn(
        &state.display_handle,
        None,
        std::iter::empty::<(String, String)>(),
        true,
        Stdio::null(),
        Stdio::null(),
        |_| (),
    ) {
        Ok(spawned) => spawned,
        Err(err) => {
            crate::diagnostics::log(format!("xwayland:spawn_failed err={err}"));
            tracing::warn!("failed to spawn native XWayland: {err}");
            return;
        }
    };
    crate::diagnostics::log("xwayland:spawn_ok");

    let display_handle = state.display_handle.clone();
    if let Err(err) =
        state
            .loop_handle
            .insert_source(xwayland, move |event, _, data: &mut DriftWm| match event {
                XWaylandEvent::Ready {
                    x11_socket,
                    display_number,
                } => {
                    crate::diagnostics::log(format!("xwayland:ready display=:{display_number}"));
                    let display_name = format!(":{display_number}");
                    match X11Wm::start_wm(
                        data.loop_handle.clone(),
                        &display_handle,
                        x11_socket,
                        client.clone(),
                    ) {
                        Ok(wm) => {
                            crate::diagnostics::log(format!(
                                "xwayland:xwm_start_ok display={display_name}"
                            ));
                            data.config
                                .child_env
                                .insert("DISPLAY".into(), display_name.clone());
                            unsafe { std::env::set_var("DISPLAY", &display_name) };
                            export_display(&display_name);
                            data.xdisplay = Some(display_number);
                            data.xwm = Some(wm);
                            tracing::info!("native XWayland ready on DISPLAY={display_name}");
                        }
                        Err(err) => {
                            crate::diagnostics::log(format!("xwayland:xwm_start_failed err={err}"));
                            tracing::warn!("failed to attach X11 window manager: {err}");
                        }
                    }
                }
                XWaylandEvent::Error => {
                    data.xdisplay = None;
                    data.xwm = None;
                    crate::diagnostics::log("xwayland:error");
                    tracing::warn!("native XWayland crashed during startup");
                }
            })
    {
        crate::diagnostics::log(format!("xwayland:insert_source_failed err={err}"));
        tracing::warn!("failed to insert native XWayland event source: {err}");
    } else {
        crate::diagnostics::log("xwayland:insert_source_ok");
    }
}

fn export_display(display_name: &str) {
    let cmd = "systemctl --user import-environment DISPLAY; \
               hash dbus-update-activation-environment 2>/dev/null && \
               dbus-update-activation-environment --systemd DISPLAY";
    match Command::new("/bin/sh")
        .args(["-c", cmd])
        .env("DISPLAY", display_name)
        .spawn()
    {
        Ok(mut child) => {
            if let Err(err) = child.wait() {
                tracing::warn!("error waiting for DISPLAY import: {err}");
            }
        }
        Err(err) => tracing::warn!("failed to import DISPLAY into session: {err}"),
    }
}
