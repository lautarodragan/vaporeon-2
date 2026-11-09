# File format

Vaporeon keeps its data in TOML files, in a directory that's usually a git repository:

- `YYYY-MM-DD.toml`: the tasks of a day.
- `global.toml`: the global task list.
- `template.toml`: the tasks a day can start with.

All of them have the same format. Any valid TOML with the right fields can be read, but there's exactly one way to
write a file, described here, so that every app that writes these files (e.g. on different devices) writes the same
bytes for the same tasks. Otherwise, files would change every time a different app saves them, and merging them would
conflict for no reason.

## Example

```toml
date_time = "2026-11-09"

[[tasks]]
id = "0b6c3a52-1f4e-4d1a-9c55-2f3b8a3e6d10"
text = "Buy eggs"
project_id = 0
status = "ToDo"

[[tasks]]
id = "5e0f9d7c-8a2b-4c61-b3d4-7a9e1c2f0b85"
text = "Read \"The Mythical Man-Month\""
project_id = 0
status = "ToDo"
details = """
Chapters 1 and 2.
The rest, next week."""

[[tasks]]
id = "c2a4e6f8-0b1d-4e3f-a5c7-e9b1d3f5a7c9"
text = "Pay the rent"
project_id = 2
status = "Done"
```

## Rules

The file is UTF-8, with `\n` line endings, and ends with a newline.

It starts with the date, as `date_time = "YYYY-MM-DD"`. Then come the tasks, each one an `[[tasks]]` table preceded by
an empty line, with these keys, in this order:

| Key          | Value                                                                      |
|--------------|----------------------------------------------------------------------------|
| `id`         | A UUID, lowercase, with hyphens. Unique within the file.                   |
| `text`       | The title, as a single-line string (see below).                            |
| `project_id` | The id of the project, an integer.                                         |
| `status`     | `"ToDo"`, `"Doing"`, `"Done"` or `"Dropped"`.                              |
| `details`    | The description, as a multi-line string (see below). Omitted if empty.     |

### Order of the tasks

1. By status: `ToDo`, `Doing`, `Done`, `Dropped`.
2. Within a status, by `project_id`, ascending.
3. Within a status and project, in the order the user put them in. This order is meaningful: it's not sorted.

### Tasks that aren't written

Tasks with neither a title nor a description (both empty, or only whitespace) aren't written. The app shows an empty
task in every project without tasks, so there's somewhere to start typing, but those aren't saved.

### Strings

Titles are always written as a basic string on one line: `"..."`. Descriptions are always written as a multi-line basic
string, starting with a newline right after the opening quotes, which isn't part of the text:

```toml
details = """
First line
Second line"""
```

Characters are written as they are, except for these, which are escaped:

| Character                                      | In titles  | In descriptions                           |
|------------------------------------------------|------------|-------------------------------------------|
| `\`                                            | `\\`       | `\\`                                      |
| `"`                                            | `\"`       | Only the third of three `"` in a row: `""\"` |
| Line feed (U+000A)                             | `\n`       | Written as is (a new line)                |
| Tab (U+0009)                                   | `\t`       | Written as is                             |
| Carriage return (U+000D)                       | `\r`       | `\r`                                      |
| Other control characters (U+0000 to U+001F, U+007F) | `\uXXXX` (uppercase hex) | `\uXXXX` (uppercase hex) |

In descriptions, quotes are counted from the last escaped one: in `"""""` (five quotes) the third is escaped, then the
count starts again, so it's written `""\"""`. Quotes right before the closing `"""` don't need anything special (TOML
allows up to two there).

Titles are meant to be a single line. If one has a line feed anyway, it's escaped as above.

## Reading

- `id` may be missing (files written by older versions), or repeated (e.g. a task copied by hand). The app gives those
  tasks a new id. The file is written in this format the next time it's saved, not when it's read.
- `details` may be missing, or `""`.
- Strings may be written in any TOML style (literal strings, multi-line strings, escapes).
- Tasks may be in any order. The order within a status and project is kept.
- Tasks of projects the app doesn't show (hidden, or unknown) are kept as they are.

## Ids

A task keeps its id when it's edited, moved within a file or to another file (e.g. dragged to the global list), or
copied to another day (unfinished tasks are copied to the next day). That's what tells it's the same task when merging
two versions of a file.

A task gets a new id when it's created, pasted, duplicated, or added from the template.
