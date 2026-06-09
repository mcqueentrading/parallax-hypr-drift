# Hypr Drift

<h1 align="center">
  <img alt="Hypr Drift" src="assets/parallax/logo.png" width="420">
</h1>

<p align="center">
  <strong>Hyprland-style tiling on an infinite canvas.</strong>
</p>

<p align="center">
  <img alt="Hypr Drift demo" src="assets/hypr-drift-demo.gif" width="820">
</p>

Hypr Drift is a Wayland compositor fork built from DriftWM and pushed toward a
Hyprland-like workflow: tiled workspaces, fast keyboard control, cursor-aware
window placement, and an infinite canvas that lets workspaces exist as visible
places instead of hidden slots.

This repository is the steadier public testing line. It is meant for trying the
core Hypr Drift workflow without mixing in the more experimental private
parallax work.

## The Idea

Most compositors treat workspaces like separate rooms with the lights turned
off. You switch workspace, the old context disappears, and you have to remember
where everything went.

Hypr Drift treats workspaces as regions on one larger surface:

- `SUPER+1` through `SUPER+6` moves the camera to a workspace zone.
- `SUPER+O` zooms out to the six-workspace overview.
- Windows tile inside their workspace rather than floating randomly.
- Moving a window between workspaces keeps the desktop feeling spatial.
- The cursor position matters, so new windows open where the user is working.

The target is practical first, visual second: a compositor that can be used
like Hyprland, but with the extra freedom of an infinite map.

## Status

This is not a polished stable release. It is the stabler testing branch.

Use it if you want to test the workflow, report breakage, or follow the
development direction. Do not install it expecting a finished daily-driver
compositor.

## Vibe-Code Disclaimer

We exclusively vibe-code this project.

That means the codebase is built with AI assistance, then manually read,
tested, argued with, broken, fixed, and reshaped. I am good at programming,
terrible at writing code from scratch, and much better at reading code than my
commit history probably deserves.

Use at your own risk. If it breaks, keep both pieces and send logs.

## Current Features

- Six workspace zones on one infinite canvas.
- Hyprland-inspired dwindle tiling inside each workspace.
- Per-workspace tiling state, instead of one global rectangle shuffle.
- `SUPER+1` through `SUPER+6` jumps to workspace zones.
- `SUPER+SHIFT+1` through `SUPER+SHIFT+6` moves the hovered/focused window to a
  workspace and retiles it there.
- `SUPER+V` toggles focused window floating/tiled.
- `SUPER+J` toggles the active dwindle split direction.
- `SUPER+T` forces normal windows in the current workspace back into tiling.
- `SUPER+O` shows the six-zone overview.
- `SUPER+A` jumps to workspace 1 and back to the previous canvas position.
- Multi-monitor viewports can look at different parts of the same canvas.
- XWayland work is in progress so X11 apps behave more predictably.

## Workspace Model

Default layout:

```text
1  2  3
4  5  6
```

Each number is a real rectangle on the canvas. The camera moves between these
rectangles, but the larger desktop still exists around them.

This is the important difference: workspaces are not just hidden pages. They
are places.

## Key Bindings

`Mod` means `SUPER` in the example config.

| Binding | Action |
| --- | --- |
| `Mod+1` .. `Mod+6` | Jump camera to workspace zone |
| `Mod+Shift+1` .. `Mod+Shift+6` | Move hovered/focused window to workspace |
| `Mod+O` | Toggle six-workspace overview |
| `Mod+A` | Toggle workspace 1 / previous canvas position |
| `Mod+V` | Toggle focused window floating/tiled |
| `Mod+J` | Toggle dwindle split direction |
| `Mod+T` | Retile current workspace |
| `Mod+W` | Zoom to fit visible windows |
| `Mod+Q` / `Mod+Return` | Launch terminal in the example config |
| `Mod+C` | Close focused window in the example config |
| `Print` / `Mod+P` | Full screenshot in the example config |
| `Shift+Print` / `Mod+Shift+P` | Area screenshot in the example config |

Public example config:

```text
parallax-hypr-drift/config.toml
```

The folder name still reflects the old working name. The public project name is
now Hypr Drift.

## Multi-Monitor Behavior

External monitors can look at the same infinite canvas from different
positions. If the pointer is on monitor 1 and you press `SUPER+2`, monitor 1
moves to workspace 2. If the pointer is on monitor 2 and you press `SUPER+4`,
monitor 2 moves to workspace 4.

That gives the desktop a useful split-brain mode: one monitor can stay on code
while another monitors logs, browser output, or a different workspace cluster.

## Build

Requires Rust 1.88+.

Arch dependencies:

```bash
sudo pacman -S libdisplay-info libinput seatd mesa libxkbcommon
```

Build:

```bash
git clone https://github.com/mcqueentrading/hypr-drift.git
cd hypr-drift
cargo build --release
```

Run directly:

```bash
./target/release/driftwm --config parallax-hypr-drift/config.toml
```

The binary is still named `driftwm` for now. A full rename has to be handled
carefully because compositor names touch configs, sessions, services, portals,
IPC, package names, and user startup scripts.

## Project Split

This repo:

```text
https://github.com/mcqueentrading/hypr-drift
```

This is the more stable public testing line.

The deeper parallax experiments belong in a separate repo and should not be
mixed into this one:

```text
https://github.com/mcqueentrading/hypr-drift-parallax
```

## Credit

Hypr Drift is based on DriftWM by malbiruk:

```text
https://github.com/malbiruk/driftwm
```

It takes heavy workflow inspiration from Hyprland by Vaxry:

```text
https://github.com/hyprwm/Hyprland
https://hypr.land
```

The depth/parallax direction is also inspired by `neorx_`, whose work helped
shape the idea of treating the desktop as a spatial surface rather than only a
flat workspace switcher.

DriftWM gave this project the editable infinite-canvas base. Hyprland gave the
reference point for tiling, focus, animation feel, and daily-driver ergonomics.
Hypr Drift is the attempt to merge those instincts into one compositor.

## License

GPL-3.0-or-later
