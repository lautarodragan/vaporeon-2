# Vaporeon

Bullet Journal and stuff. GTK 4 port of [vaporeon](https://github.com/lautarodragan/vaporeon) (which was a terminal app).

Everything works the same as in the terminal version: same keyboard shortcuts, same files.
It reads and writes the same `settings.toml` and `actions.ini` (in `$XDG_CONFIG_HOME/vaporeon`, which is
`~/.config/vaporeon` by default) and the day files in `storage_path` (`~/vaporeon` by default), so both versions can be used interchangeably.

## Saving

The day is saved (and committed, if `storage_path` is a git repository) with `Ctrl+S`, when going to another day,
and when closing the window.

## Mouse

Everything can still be done with the keyboard, but you can also:

- click a task to select it,
- click a day of the week, in the header, to go to that day (or the arrows, to go back to today),
- double click a task to edit it,
- click a task's description, in the details panel (`D`), to edit it,
- scroll with the mouse wheel or touchpad,
- drag and drop tasks: to reorder them, or to move them to another project, status (e.g. from TODO to DONE), or
  to the other panel (between the day and the global list). Dragging near the top or bottom edge scrolls the list.
  Text dropped from other applications becomes a new task (first line: title, rest: description).

Clicking elsewhere while editing confirms the edit.

## Editing text

Text is edited with regular GTK text widgets, placed exactly where the text is drawn. So all the usual GTK
shortcuts work while editing (Ctrl+Backspace, Ctrl+A, Shift+arrows, Ctrl+Z, etc.), as well as dead keys,
compose sequences and input methods.

`actions.ini` only controls entering and leaving edit mode (`Text.EnterEditMode`, `Text.ConfirmEdit`,
`Text.ConfirmEditMultiLine` and `Text.CancelEdit`). Cancelling an edit restores the previous text.

## Building

Requires GTK 4.12 or newer.

```sh
# Arch
sudo pacman -S gtk4
# Debian / Ubuntu
sudo apt install libgtk-4-dev

cargo build --release
```

The toolchain is pinned in `rust-toolchain.toml` (rustup installs it automatically).
Formatting needs nightly, because `rustfmt.toml` uses unstable options: `cargo +nightly fmt` (or `just fmt`).

## Transparency

The window itself is fully transparent; Vaporeon paints its own background using `background` and
`background_opacity` from the theme. Text is always opaque.

- `background_opacity = 0.0`: fully transparent (only the text is drawn).
- `background_opacity = 1.0`: fully opaque.

Hyprland (or any compositor with blur) will blur what's behind the window as usual.

## Theme

Copy [`assets/theme.toml`](assets/theme.toml) to `~/.config/vaporeon/theme.toml` (or `$XDG_CONFIG_HOME/vaporeon/theme.toml`)
and edit it.
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
