# Freeze Diagnostics Logging

This file records the temporary diagnostics added while tracking hard freezes in
Parallax Hypr Drift. Keep it updated while adding or removing freeze/debug logs.

## Log File

Default persistent log:

```sh
/home/unknown/Documents/scripts/projectcampaign/parallax-hypr-drift-freeze.log
```

Override path:

```sh
DRIFTWM_DIAG_LOG=/tmp/parallax-hypr-drift-freeze.log driftwm
```

Each line is flushed immediately so the final event should survive a hard
freeze better than normal buffered stdout.

Line format:

```text
<unix_seconds>.<millis> pid=<pid> <event>
```

Quick inspection after reboot:

```sh
tail -n 300 /home/unknown/Documents/scripts/projectcampaign/parallax-hypr-drift-freeze.log
rg 'heartbeat|tile:|xdg:|compositor:|grab:' /home/unknown/Documents/scripts/projectcampaign/parallax-hypr-drift-freeze.log
```

To clear the log without deleting the file:

```sh
: > /home/unknown/Documents/scripts/projectcampaign/parallax-hypr-drift-freeze.log
```

## Code Locations

Main diagnostics helper:

```text
src/diagnostics.rs
```

Current call sites:

```text
src/main.rs
src/handlers/xdg_shell.rs
src/handlers/compositor.rs
src/grabs/move_grab.rs
src/state/tile.rs
```

## Event Families

Startup and heartbeat:

```text
startup: driftwm diagnostics online ...
event_loop:start ...
heartbeat windows=... active_ws=... zoom=... camera=(...) floating=... focus_history=...
```

XDG shell lifecycle:

```text
xdg:new_toplevel ...
xdg:new_toplevel_anchor ...
xdg:destroy_toplevel ...
xdg:unmap_prepare ...
xdg:unmap_done ...
```

First map and initial tiling:

```text
compositor:first_map ...
compositor:first_map_skip_tile ...
```

Move/reconcile:

```text
grab:move_release ...
tile:reconcile_moved ...
```

Tiling and workspace rebuilds:

```text
tile:toggle_floating_to_tiled ...
tile:toggle_floating_to_float ...
tile:move_to_workspace ...
tile:prepare_unmap ...
tile:retile_after_unmap ...
tile:current_workspace_begin ...
tile:current_workspace_skip ...
tile:current_workspace_collect ...
tile:workspace_begin ...
tile:workspace_end ...
```

## How To Read Freeze Evidence

If heartbeat stops at the freeze time, the compositor event loop or full machine
likely stopped.

If heartbeat continues while the UI appears frozen, the issue is more likely in
rendering, input routing, focus, or window damage rather than total event-loop
death.

If the last event is a `tile:*` event with a high `elapsed_ms`, focus on tiling,
retile, configure/remap churn, or workspace membership loops.

If `tile:workspace_begin` and `tile:workspace_end` repeat very rapidly, look for
an event loop caused by configure acknowledgements, remaps, or repeated retile
requests.

If the last event is `xdg:*` or `compositor:first_map`, focus on new-window
mapping, cursor-anchor split logic, and initial placement.

## Cleanup Checklist

When the freeze issue is solved and the diagnostics are no longer needed:

1. Delete `src/diagnostics.rs`.
2. Remove `mod diagnostics;` from `src/main.rs`.
3. Remove startup, event-loop, and heartbeat calls from `src/main.rs`.
4. Remove `use driftwm::window_ext::WindowExt;` from `src/main.rs` if it is only
   used by the heartbeat.
5. Remove `crate::diagnostics::log(...)` calls from:
   `src/handlers/xdg_shell.rs`, `src/handlers/compositor.rs`,
   `src/grabs/move_grab.rs`, and `src/state/tile.rs`.
6. Remove `use std::time::Instant;` from `src/state/tile.rs` if timing is no
   longer used.
7. Delete or ignore the persistent log file:
   `/home/unknown/Documents/scripts/projectcampaign/parallax-hypr-drift-freeze.log`.
8. Run `cargo fmt` and `cargo check`.

## Notes

The diagnostics are intentionally simple and file-based because the user can
hard-freeze the session and may not be able to switch TTYs. Avoid replacing this
with stdout-only logs until the freeze is fixed.

`docs/lua-config-migration.md` is a private planning note and is intentionally
not part of this diagnostics cleanup list.
