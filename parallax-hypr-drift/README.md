# Parallax Hypr Drift Config

This folder contains the public config assets for the Parallax Hypr Drift fork.
It is intentionally generic: private machine paths, personal app shortcuts, and
virtual-terminal login wiring should stay outside this Git repo.

## Project Goal

Parallax Hypr Drift is intended to merge the parts of Hyprland that work best
for daily use with DriftWM's infinite canvas model.

The target is not "DriftWM with optional tiling". The target is Hyprland-style
tiling as the foundation, running on an infinite canvas. New windows should
split the tiled area under the cursor, focus should follow the cursor, and the
layout should feel like Hyprland's dwindle behavior while still allowing the
workspace to pan, zoom, and drift beyond a fixed monitor-sized desktop.

The reason this fork starts from DriftWM rather than Hyprland is practical.
Hyprland already has the mature tiling, input, and desktop behavior we want,
but its internals are tightly coupled around its own compositor model. Our
earlier parallax/infinite-canvas experiments showed that changing one part of
Hyprland can break unrelated behavior because much of the compositor state is
deeply integrated. DriftWM is easier for us to reshape: its infinite canvas is
already native, and the codebase has been more approachable for AI-assisted
iteration.

So the design direction is:

- Keep tiling mandatory for normal windows.
- Treat floating as an escape hatch for special cases, not as the default mode.
- Copy Hyprland's behavior where it is proven: dwindle-like splitting,
  cursor-led focus, reliable keybinds, app launching, XWayland compatibility,
  and normal desktop session startup.
- Keep DriftWM's strength: infinite canvas, camera movement, zoom, parallax,
  and experimental visual space.

## What This Adds

- Mandatory Hyprland-inspired tiling: new windows split the tile under the cursor.
- Cursor-led focus: moving the pointer over a window selects it.
- Floating escape hatch: temporarily move a special-case window out of the tiled layout.
- Direct screenshot shortcuts using `grim` and `slurp`.
- Matrix/parallax-style DriftWM shader background.
- Stronger DBus/Wayland session environment for apps like Chrome and Codex.

## Files

- `config.toml` - public baseline DriftWM config for the parallax session.
- `parallax_matrix_space.glsl` - custom background shader.
- `start-parallax-hypr-drift` - generic launcher example.

Private local configs should use names like `config.private.toml` or
`config.local.toml`; these are ignored by Git.

## Build

From the repo root:

```bash
cargo build --release
```

The patched binary will be:

```bash
target/release/driftwm
```

## Example Install

Copy the public config and shader:

```bash
mkdir -p ~/.config/parallax-hypr-drift
cp parallax-hypr-drift/config.toml ~/.config/parallax-hypr-drift/config.toml
cp parallax-hypr-drift/parallax_matrix_space.glsl ~/.config/parallax-hypr-drift/parallax_matrix_space.glsl
```

Edit this line in `~/.config/parallax-hypr-drift/config.toml`:

```toml
path = "/home/YOUR_USER/.config/parallax-hypr-drift/parallax_matrix_space.glsl"
```

Replace `YOUR_USER` with your Linux username or use another absolute path.

Run manually for testing:

```bash
dbus-run-session ./target/release/driftwm --backend udev --config ~/.config/parallax-hypr-drift/config.toml
```

Or copy the launcher:

```bash
mkdir -p ~/.local/bin
cp parallax-hypr-drift/start-parallax-hypr-drift ~/.local/bin/start-parallax-hypr-drift
chmod +x ~/.local/bin/start-parallax-hypr-drift
```

## Keybinds In The Public Config

- `SUPER+Q` / `SUPER+Return` - open kitty.
- `SUPER+C` - close focused window.
- `SUPER+G` - open Google Chrome.
- `SUPER+F` - toggle fullscreen.
- `SUPER+V` - toggle focused window floating/tiled.
- `SUPER+O` - toggle six-workspace overview / previous position.
- `SUPER+Ctrl+M` - toggle independent/mirrored monitor viewports.
- `Alt+1` .. `Alt+0` - move cursor/focus to monitor 1..10 by layout order.
- `SUPER+W` - zoom to fit current windows.
- `SUPER+Left Click` - move floating window.
- `SUPER+Right Click` - resize floating window.
- `Print` / `SUPER+P` - full screenshot.
- `SHIFT+Print` / `SUPER+SHIFT+P` - area screenshot.

Screenshots save to:

```bash
$HOME/Pictures
```
