# Parallax Hypr Drift

Local experimental DriftWM setup for the TTY4 `parallax-hypr-drift` session.

## Files

- `config.toml` - TTY4 DriftWM config with tiling, floating toggle, screenshots, app binds, and parallax shader.
- `parallax_matrix_space.glsl` - Matrix-style DriftWM background shader.
- `parallax-hypr-drift-tty4-session` - TTY4 launcher wrapper.
- `driftwm-tty3-session` - normal TTY3 launcher wrapper using the patched compositor binary.

## Local Paths

Live config:

```bash
/home/unknown/Documents/scripts/projectcampaign/parrlax-hypr-drift/config.toml
```

Patched source:

```bash
/home/unknown/Documents/scripts/projectcampaign/driftwm-src
```

Build:

```bash
cd /home/unknown/Documents/scripts/projectcampaign/driftwm-src
cargo build --release
```

TTY4 starts through:

```bash
/home/unknown/.local/bin/parrlax-hypr-drift-tty4-session
```

## Current Keybinds

- `SUPER+Q` - kitty
- `SUPER+S` / `SUPER+D` - tlauncher through kitty
- `SUPER+C` - close window
- `SUPER+F` - fullscreen
- `SUPER+V` - toggle floating
- `SUPER+Left Click` - move floating window
- `SUPER+Right Click` - resize floating window
- `Print` / `SUPER+P` - screenshot to `/home/unknown/Pictures`
- `SHIFT+Print` / `SUPER+SHIFT+P` - area screenshot to `/home/unknown/Pictures`

