# Vaporeon

Bullet Journal and stuff. GTK 4 port of [vaporeon](https://github.com/lautarodragan/vaporeon) (which was a terminal app).

Everything works the same as in the terminal version: same keyboard shortcuts, same files.
It reads and writes the same `settings.toml` and `actions.ini` (in `$XDG_CONFIG_HOME/vaporeon`, which is
`~/.config/vaporeon` by default) and the day files in `storage_path` (`~/vaporeon` by default), so both versions can be used interchangeably.

## Saving

The day is saved (and committed, if `storage_path` is a git repository) with `Ctrl+S`, when going to another day,
and when closing the window.

A `•` next to the date means the day has unsaved changes. Messages (like "Saved", or errors) show up at the bottom.

### Syncing with a git remote

If `storage_path` is a git repository with a remote (e.g. a private GitHub repository), Vaporeon keeps it in sync:
it commits whatever isn't committed yet, pulls (merging) and pushes. When starting, a minute after saving (so a
burst of changes is pushed once), every five minutes, when the window gets the focus, and when closing. Git runs
in the background, so the UI never waits for the network.

So tasks can be read and added from anywhere: GitHub's web editor, any git client, or an AI assistant with access to
the repository. Changes pulled from the remote are handled like any other change made outside of Vaporeon (see
below).

- No remote, no syncing.
- Git never asks for anything (passwords, passphrases): it has to work non-interactively, e.g. with an ssh agent or
  a credential helper. Unreachable remotes time out.
- If your changes and the remote's conflict, the merge is aborted (the repository is never left in the middle of
  one) and a message asks you to merge them with git.
- Problems are shown at the bottom, once (not again and again while offline, for example).

### Changes made outside of Vaporeon

The files of the selected day and of the global list are watched, so editing them in another program (or a
`git pull`) is noticed right away:

- If you don't have unsaved changes, the file is reloaded.
- If you do, a question at the bottom asks what to do: `r` reload (discarding your changes), `k` keep yours (saving
  will overwrite the file), or `b` save yours to a backup (`<day>.toml.bak-<time>`, not committed) and reload.
  Until you answer, nothing else responds, so nothing overwrites the file by accident. Closing the window
  meanwhile saves your version to a backup too, instead of overwriting the file.

## Mouse

Everything can still be done with the keyboard, but you can also:

- click a task to select it,
- click a day of the week, in the header, to go to that day (or the arrows, to go back to today),
- double click a task to edit it,
- click a task's description, in the details panel (`Space`), to edit it,
- scroll with the mouse wheel or touchpad,
- drag and drop tasks: to reorder them, or to move them to another project, status (e.g. from TODO to DONE), or
  to the other panel (between the day and the global list). Dragging near the top or bottom edge scrolls the list.
  Text dropped from other applications becomes a new task (first line: title, rest: description).

Clicking elsewhere while editing confirms the edit.

## Editing text

Text is edited with regular GTK text widgets, placed exactly where the text is drawn. So all the usual GTK
shortcuts work while editing (Ctrl+Backspace, Ctrl+A, Shift+arrows, Ctrl+Z, etc.), as well as dead keys,
compose sequences and input methods.

`actions.ini` only controls entering and leaving edit mode: `Task.EditTitle` and `Task.EditDescription` enter it,
and `Text.ConfirmEdit` and `Text.CancelEdit` (for titles) or `Text.ConfirmEditMultiLine` and
`Text.CancelEditMultiLine` (for descriptions) leave it. Cancelling an edit restores the previous text.

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

Tasks with details (a description) can be told apart without opening the details panel. `details_indicator` chooses
how: a dimmed preview of the details after the title (`preview`, the default), a different bullet (`bullet`, with
`bullet_with_details`), a dot in the margin (`gutter`), or a mark at the end of the row (`right`, with `details_marker`).
None of them moves the text.

Besides colors, the theme has `font` (any Pango font description), `line_height`, `details_wrap` (whether long
lines of the details wrap; on by default) and `window_decorations` (GTK title bar; off by default).

## Starting a new day

When opening today's (empty) page, if the last day with tasks (looking back up to two weeks, so Mondays find
Fridays) has unfinished ones (to do or doing), a question at the bottom offers to copy them: `y` copies them, `n` or
`Esc` doesn't. Like bullet journal's migration. `Ctrl` `P` copies yesterday's unfinished tasks any time.

## Undo

`Ctrl` `Z` undoes the last change, and `Ctrl` `Shift` `Z` (or `Ctrl` `Y`) redoes it. Any change can be undone: adding,
deleting, editing or moving tasks, drag and drop, etc. Editing a task, from starting to confirming, is a single step
(while editing, `Ctrl` `Z` undoes text changes instead, like in any text box).

The history starts over when going to another day, or when a file is reloaded because it changed outside of Vaporeon.

## Zoom

`Ctrl` `+` (or `Ctrl` `=`) and `Ctrl` `-` make everything bigger or smaller (text, rows, spacing), like in a browser;
`Ctrl` `0` goes back to 100%. So does `Ctrl` + mouse wheel. The zoom is remembered between runs.

## Window size

The window size (and whether it's maximized) is remembered between runs, like the zoom, in `$XDG_STATE_HOME/vaporeon/state.toml`
(`~/.local/state/vaporeon/state.toml` by default). On tiling compositors this only matters for floating windows,
and window rules (e.g. Hyprland's `size`) take precedence.

## Hyprland

The window's class (app id) is `codes.taro.Vaporeon`, e.g.:

```
windowrulev2 = float, class:^(codes\.taro\.Vaporeon)$
windowrulev2 = size 1200 800, class:^(codes\.taro\.Vaporeon)$
```

Only one instance runs at a time: launching it again focuses the existing window.

## Logs

Logs are written to `$XDG_STATE_HOME/vaporeon/vaporeon.log` (`~/.local/state/vaporeon/vaporeon.log` by default). `--log-events` logs every key press, which
is useful when writing key bindings in `actions.ini`.
