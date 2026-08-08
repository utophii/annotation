# annotation

A transparent full-screen "painter," like Whiteboard/Paint, on top of all windows.
Launched with a hotkey, drawn with the mouse, and exited with 'Esc'.

Written in **Rust** with 'gtk4' + 'gtk4-layer-shell' (the Wayland layer-shell protocol).
Works with any Wayland compositor that supports 'wlr-layer-shell' (Niri, Sway, Hyprland, etc.).

## Build (Arch Linux)

```bash
sudo pacman -S gtk4 gtk4-layer-shell cairo pkgconf
cargo build --release
# binary: target/release/annotation
cp target/release/annotation ~/.local/bin/ # or wherever convenient, in $PATH
```

## Hotkey binding in Niri

Add the binding to `~/.config/niri/config.kdl` (the config in niri is automatically reloaded when you save the file):

```kdl
binds {
Super+Z { spawn "annotation"; }
}
```

Now `Super+Z` → the overlay appears, draw with the mouse.
`Esc` (or `Q`) → exit mode (the process ends).

## Controls within mode

| Key | Action |
|--------------|------------------------------|
| `Esc` / `Q` | quit |
| `C` | clear all |
| `+` / `-` | pen thickness |
| LMB + drag | draw a line |
| palette button in the top-right panel | color picker |

In the top-right corner of the overlay, there's a small translucent panel with:
a color picker button, `-`/`+` thickness buttons, a "Clear" button, and an "Exit" button-in case you prefer a mouse rather than a keyboard.

## How it works

- `window.init_layer_shell()` turns a GTK window into a **layer-surface**
(the `wlr-layer-shell` protocol, which Niri supports).
- `Layer::Overlay` + `set_anchor` to all 4 edges → the window stretches to the entire
output and lies on top of regular windows.
- `KeyboardMode::Exclusive` → the overlay captures the entire keyboard while

open, so `Esc`/letters don't escape into the application below.
- `set_exclusive_zone(0)` → the overlay doesn't push adjacent windows/panels.
- Transparent CSS window background (`background-color: transparent`) + redrawing
every frame via `cairo::Operator::Clear` at the beginning of `draw_func` → the desktop is visible through

unfinished areas.
- Strokes are stored as `Vec<Stroke>`, where `Stroke` is the color, thickness, and list of

points `(x, y)`. Mouse drawing is implemented via `GestureDrag`
(`drag-begin` creates a new stroke, `drag-update` adds points to it,
`drag-end` commits the stroke to the master list). All state lives in
`Rc<RefCell<DrawState>>` and is redrawed via `DrawingArea::queue_draw`.

## Known Limitations

- Multi-monitor: By default, the overlay appears on the "current" output,
selected by the compositor. You can add an explicit monitor selection using
`LayerShell::set_monitor`.
- Doesn't save the drawing to disk-it's a one-time "whiteboard over the screen";
strokes only last as long as the process is running.