# Parallax Hypr Drift

<h1 align="center"><img alt="Parallax Hypr Drift" src="assets/parallax/logo.png" width="420"></h1>

Parallax Hypr Drift is an experimental Wayland compositor fork. The goal is
simple: keep DriftWM's infinite canvas, but make it behave more like Hyprland
for everyday desktop work.

The current direction is **Hyprland-style workspaces on one continuous canvas**.
Instead of hiding workspaces off-screen, six zones exist together on the same
infinite plane. You can jump between them like workspaces, zoom out to see the
larger map, and later bend that map into parallax/cube views.

This is experimental software. It is built for rapid compositor research, not a
stable daily-driver promise.

## Current Focus

- Six visible workspace zones on one infinite canvas.
- Hyprland-inspired tiled window placement.
- New tiled windows spawn under the cursor and split the tile under the cursor.
- Normal windows are expected to tile inside their current workspace zone.
- Floating is explicit through `toggle-floating`, not the default behavior.
- Moving a window between zones retiles it with the windows already there.
- Closing or floating a tiled window should make the remaining windows fill the
  gap.
- Stronger graphical session environment export for DBus, Wayland, XDG portals,
  and XWayland-style app launches.
- Persistent freeze diagnostics while the compositor is being hardened.

## Why This Fork Exists

Hyprland already has excellent tiling behavior, focus behavior, and practical
desktop ergonomics. The missing piece for this project is an infinite canvas
where workspace areas can remain spatially visible instead of being hidden when
inactive.

DriftWM already provides the infinite canvas base. Parallax Hypr Drift adds the
Hyprland-like parts we need on top: workspace zones, mandatory tiling, cursor
anchored placement, window movement between zones, and a future path toward
parallax projection.

## Workspace Model

The compositor currently supports six workspace zones.

Default layout:

```text
1  2  3
4  5  6
```

Each zone is a workspace-sized rectangle on the infinite canvas. `Mod+1` through
`Mod+6` moves the camera to that zone. Windows inside that zone tile within that
zone's boundaries.

Future parallax/cube layout:

```text
        4
1       2       3
        5
        6
```

The long-term idea is that normal mode can use the flat `1 2 3 / 4 5 6` grid,
while parallax mode can reinterpret the same six stable workspace IDs as a cube
net:

- `2` is front/center.
- `1` is left.
- `3` is right.
- `4` is above.
- `5` is below.
- `6` is behind.

Window ownership should stay stable. Parallax should change how the canvas is
projected, not force windows to be reassigned.

## Key Bindings

`Mod` means Super by default.

| Binding | Action |
| --- | --- |
| `Mod+1` .. `Mod+6` | Jump camera to workspace zone |
| `Mod+Shift+1` .. `Mod+Shift+6` | Move hovered/focused window to zone and retile |
| `Mod+V` | Toggle focused window floating/tiled |
| `Mod+T` | Force all windows in the current zone back into tiling |
| `Mod+W` | Zoom out / overview |
| `Mod+Q` | Launch terminal in the example config |
| `Mod+C` | Close focused window in the example config |
| `Print` | Screenshot to clipboard in the example config |
| `Shift+Print` | Region screenshot to clipboard in the example config |

Check the config for exact local bindings. Public example config lives under:

```text
parallax-hypr-drift/
```

## Tiling Behavior

The tiler is being shaped around Hyprland's practical behavior:

- Spawn in the workspace under the cursor when in workspace perspective.
- If the cursor is over an existing tiled window, split that tile.
- If no tile is under the cursor, fall back to focused/largest tile.
- Keep tiled windows bounded by their workspace rectangle.
- When a window leaves a workspace, retile the remaining windows so there is no
  empty hole.
- If a dragged tiled window is released inside the same workspace, snap it back
  to its existing tile instead of rebuilding the whole layout.

Floating windows are treated as exceptions. The intended model is tiled by
default, floating only when the user explicitly asks for it.

## Configuration Today

Current config is still TOML-compatible with the upstream base.

Default config path:

```text
~/.config/driftwm/config.toml
```

Useful fork-specific options include:

```toml
workspace_layout = "grid"      # current normal layout
window_placement = "tile"      # current parallax/hypr workflow
focus_follows_mouse = true
```

The public example folder contains a fork-oriented config, shader, and launcher
pieces:

```text
parallax-hypr-drift/
```

Machine-specific login/session wiring should stay outside the public repo.

## Lua Config Path

TOML is useful for simple settings, but this fork is moving toward a
Hyprland-style programmable config layer.

Target direction:

- Keep TOML compatibility while the compositor stabilizes.
- Add optional `config.lua`.
- Lua builds the same internal config structure Rust uses today.
- Lua handles expressive user setup: binds, startup commands, workspace layout,
  parallax mode toggles, theme logic, and future animation settings.
- Rust remains authoritative for compositor state, Wayland safety, window
  ownership, tiling validity, and final config validation.

Example direction:

```lua
drift.mod_key("super")
drift.workspace_layout("grid")
drift.window_placement("tile")
drift.focus_follows_mouse(true)

drift.bind("mod+q", "spawn kitty")
drift.bind("mod+c", "close-window")
drift.bind("mod+v", "toggle-floating")
drift.bind("mod+t", "tile-current-workspace")

drift.command("parallax-mode", function()
  drift.workspace_layout("cube-net")
  drift.parallax.enable_cube_projection()
end)

drift.bind("mod+p", "lua parallax-mode")
```

Lua should configure and request behavior. Rust should decide what is valid.

## Roadmap

### Stage 1: Workspace Zones

Status: in progress.

- Six stable workspace IDs.
- Camera jumps with `Mod+1..6`.
- Window movement with `Mod+Shift+1..6`.
- Grid layout now, cube-net projection later.

### Stage 2: Mandatory Workspace Tiling

Status: in progress.

- Tiled windows fill the current workspace zone.
- New windows split the tile under the cursor.
- Close/unmap/floating transitions retile the remaining windows.
- `Mod+T` forces the current zone back into tiling.

### Stage 3: Hyprland-Like App Launch Reliability

Status: in progress.

- Export DBus, Wayland, DISPLAY, XDG, and session environment properly.
- Make launched apps work consistently without first starting another desktop.
- Improve XWayland app behavior and browser launch behavior.

### Stage 4: Lua Configuration

Status: planned.

- Add optional Lua config loader.
- Keep TOML during transition.
- Move user-facing orchestration out of hard-coded Rust where safe.

### Stage 5: Parallax Projection

Status: planned.

- Keep workspace IDs stable.
- Add a parallax/cube mode that projects the same workspace map differently.
- Build animation and depth effects around the existing infinite canvas.

### Stage 6: Visual Polish

Status: planned.

- Animated borders.
- Cleaner workspace outlines.
- Better focused-window effects.
- More intentional shader/background presets.

## Diagnostics

Temporary freeze diagnostics are documented here:

```text
docs/freeze-diagnostics.md
```

Default freeze log:

```text
/home/unknown/Documents/scripts/projectcampaign/parallax-hypr-drift-freeze.log
```

That log is intentionally file-based and flushed line-by-line because hard
freezes can prevent normal journal inspection.

## Build

Requires Rust 1.88+.

Arch dependencies:

```bash
sudo pacman -S libdisplay-info libinput seatd mesa libxkbcommon
```

Build:

```bash
git clone https://github.com/mcqueentrading/parallax-hypr-drift.git
cd parallax-hypr-drift
cargo build --release
```

Run the built compositor directly:

```bash
./target/release/driftwm --config parallax-hypr-drift/config.toml
```

For development checks:

```bash
cargo fmt
cargo check
```

## Useful Runtime Tools

- `xwayland-satellite` for X11 apps.
- `xdg-desktop-portal` and `xdg-desktop-portal-wlr` for portals/screencast.
- `grim`, `slurp`, and `wl-clipboard` for screenshots.
- `kitty`, `foot`, or another Wayland terminal.
- `fuzzel`, `wofi`, or another launcher.

## Upstream Credit

This project is based on DriftWM by malbiruk.

Upstream:

```text
https://github.com/malbiruk/driftwm
```

The original DriftWM provides the infinite-canvas Wayland compositor base. This
fork changes the workflow direction toward Hyprland-like tiling, six visible
workspace zones, parallax experiments, and programmable configuration.

## License

GPL-3.0-or-later
