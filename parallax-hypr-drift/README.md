# Parallax Hypr Drift Config

This folder contains the public config assets for the Parallax Hypr Drift fork.
It is intentionally generic: private machine paths, personal app shortcuts, and
virtual-terminal login wiring should stay outside this Git repo.

## What This Adds

- Hyprland-inspired tiling: new windows split the tile under the cursor.
- Cursor-led focus: moving the pointer over a window selects it.
- Floating toggle: move a window out of the tiled layout and back in again.
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
- `SUPER+W` - zoom to fit all windows.
- `SUPER+Left Click` - move floating window.
- `SUPER+Right Click` - resize floating window.
- `Print` / `SUPER+P` - full screenshot.
- `SHIFT+Print` / `SUPER+SHIFT+P` - area screenshot.

Screenshots save to:

```bash
$HOME/Pictures
```

