# Parallax Hypr Drift Config

This folder contains the custom session setup for the Parallax Hypr Drift fork.
It is meant as an example setup you can copy into your own Linux user config.

## What This Adds

- Hyprland-inspired tiling: new windows split the tile under the cursor.
- Cursor-led focus: moving the pointer over a window selects it.
- Floating toggle: move a window out of the tiled layout and back in again.
- Direct screenshot shortcuts using `grim` and `slurp`.
- Matrix/parallax-style DriftWM shader background.
- Stronger DBus/Wayland session environment for apps like Chrome and Codex.

## Files

- `config.toml` - DriftWM config for the parallax session.
- `parallax_matrix_space.glsl` - custom background shader.
- `parallax-hypr-drift-tty4-session` - example launcher for a separate test session.
- `driftwm-tty3-session` - example launcher for a normal DriftWM session using this patched binary.

## What TTY3 And TTY4 Mean

TTY means Linux virtual terminal. You switch between them with keys like:

```text
Ctrl+Alt+F1
Ctrl+Alt+F2
Ctrl+Alt+F3
Ctrl+Alt+F4
```

In the original local setup:

- TTY3 was used for a normal DriftWM session.
- TTY4 was used for the experimental `parallax-hypr-drift` session.

You do not have to use those exact terminals. They are just examples for
keeping a stable session and an experimental session separate.

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

Copy the config somewhere stable:

```bash
mkdir -p ~/.config/parallax-hypr-drift
cp parallax-hypr-drift/config.toml ~/.config/parallax-hypr-drift/config.toml
cp parallax-hypr-drift/parallax_matrix_space.glsl ~/.config/parallax-hypr-drift/parallax_matrix_space.glsl
```

Then update the shader path inside `config.toml` if your path is different.

Run it manually for testing:

```bash
dbus-run-session ./target/release/driftwm --backend udev --config ~/.config/parallax-hypr-drift/config.toml
```

## Keybinds In This Config

- `SUPER+Q` - open kitty.
- `SUPER+S` / `SUPER+D` - open TLauncher through kitty.
- `SUPER+C` - close focused window.
- `SUPER+F` - toggle fullscreen.
- `SUPER+V` - toggle focused window floating/tiled.
- `SUPER+Left Click` - move floating window.
- `SUPER+Right Click` - resize floating window.
- `Print` / `SUPER+P` - full screenshot.
- `SHIFT+Print` / `SUPER+SHIFT+P` - area screenshot.

Screenshots in the local config save to:

```bash
/home/unknown/Pictures
```

Change that path in `config.toml` for another user.

## Local Development Paths

These are the original local paths used while developing this fork:

```bash
/home/unknown/Documents/scripts/projectcampaign/driftwm-src
/home/unknown/Documents/scripts/projectcampaign/parrlax-hypr-drift/config.toml
/home/unknown/.local/bin/parrlax-hypr-drift-tty4-session
```

