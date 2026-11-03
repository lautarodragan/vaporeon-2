# Vaporeon

Bullet Journal and stuff. GTK 4 port of [vaporeon](https://github.com/lautarodragan/vaporeon) (which was a terminal app).

Everything works the same as in the terminal version: same keyboard shortcuts, same files.
It reads and writes the same `~/.config/vaporeon/settings.toml`, `~/.config/vaporeon/actions.ini`
and the day files in `storage_path` (`~/vaporeon` by default), so both versions can be used interchangeably.

## Building

Requires GTK 4.12 or newer.

```sh
# Arch
sudo pacman -S gtk4
# Debian / Ubuntu
sudo apt install libgtk-4-dev

cargo build --release
```

## Transparency

The window itself is fully transparent; Vaporeon paints its own background using `background` and
`background_opacity` from the theme. Text is always opaque.

- `background_opacity = 0.0`: fully transparent (only the text is drawn).
- `background_opacity = 1.0`: fully opaque.

Hyprland (or any compositor with blur) will blur what's behind the window as usual.

## Theme

Copy [`assets/theme.toml`](assets/theme.toml) to `~/.config/vaporeon/theme.toml` and edit it.
Any value you leave out falls back to the default, so you can also just write the ones you want to change:

```toml
background_opacity = 0.6
font = "Iosevka Term 14"
```

Changes to this file are applied immediately, no need to restart.

Besides colors, the theme has `font` (any Pango font description), `line_height` and `window_decorations`
(GTK title bar; off by default).

## Hyprland

The window's class (app id) is `codes.taro.Vaporeon`, e.g.:

```
windowrulev2 = float, class:^(codes\.taro\.Vaporeon)$
windowrulev2 = size 1200 800, class:^(codes\.taro\.Vaporeon)$
```

Only one instance runs at a time: launching it again focuses the existing window.

## Logs

Logs are written to `vaporeon.log` in the current directory. `--log-events` logs every key press, which
is useful when writing key bindings in `actions.ini`.
