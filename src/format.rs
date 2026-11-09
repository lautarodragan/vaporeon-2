//! Writes data files in their one canonical form, described in `docs/file-format.md`.
//!
//! Reading is done with the `toml` crate, which accepts any valid TOML.

use std::fmt::Write;

use crate::models::{Day, Task, TaskStatus};

/// The contents of the file for `day`.
pub fn day_to_string(day: &Day) -> String {
    let mut out = String::new();
    writeln!(out, "date_time = \"{}\"", day.date_time.format("%Y-%m-%d")).unwrap();

    let mut tasks: Vec<&Task> = day.tasks.iter().filter(|task| !is_blank(task)).collect();
    // Stable, so the order within a status and project (the user's) is kept.
    tasks.sort_by_key(|task| (status_order(&task.status), task.project_id));

    for task in tasks {
        out.push_str("\n[[tasks]]\n");
        if let Some(id) = task.id {
            writeln!(out, "id = \"{}\"", id.hyphenated()).unwrap();
        }
        writeln!(out, "text = \"{}\"", escape_single_line(&task.text)).unwrap();
        writeln!(out, "project_id = {}", task.project_id).unwrap();
        writeln!(out, "status = \"{}\"", status_name(&task.status)).unwrap();
        if let Some(details) = task.details.as_deref().filter(|details| !details.is_empty()) {
            writeln!(out, "details = \"\"\"\n{}\"\"\"", escape_multi_line(details)).unwrap();
        }
    }
    out
}

/// Tasks with neither a title nor a description aren't written: they're the empty placeholders the UI shows.
fn is_blank(task: &Task) -> bool {
    task.text.trim().is_empty() && task.details.as_deref().unwrap_or_default().trim().is_empty()
}

fn status_order(status: &TaskStatus) -> u8 {
    match status {
        TaskStatus::ToDo => 0,
        TaskStatus::Doing => 1,
        TaskStatus::Done => 2,
        TaskStatus::Dropped => 3,
    }
}

fn status_name(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::ToDo => "ToDo",
        TaskStatus::Doing => "Doing",
        TaskStatus::Done => "Done",
        TaskStatus::Dropped => "Dropped",
    }
}

/// The inside of a basic string (`"..."`).
fn escape_single_line(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if is_control(c) => push_unicode_escape(&mut out, c),
            c => out.push(c),
        }
    }
    out
}

/// The inside of a multi-line basic string (`"""\n..."""`).
fn escape_multi_line(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    // Unescaped quotes in a row: a third one would close the string.
    let mut quotes = 0;
    for c in text.chars() {
        if c == '"' {
            if quotes == 2 {
                out.push_str("\\\"");
                quotes = 0;
            } else {
                out.push('"');
                quotes += 1;
            }
            continue;
        }
        quotes = 0;
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' | '\t' => out.push(c),
            '\r' => out.push_str("\\r"),
            c if is_control(c) => push_unicode_escape(&mut out, c),
            c => out.push(c),
        }
    }
    out
}

fn is_control(c: char) -> bool {
    c <= '\u{1F}' || c == '\u{7F}'
}

fn push_unicode_escape(out: &mut String, c: char) {
    write!(out, "\\u{:04X}", c as u32).unwrap();
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use uuid::Uuid;

    use super::*;

    fn task(id: u128, text: &str, project_id: usize, status: TaskStatus, details: &str) -> Task {
        Task {
            id: Some(Uuid::from_u128(id)),
            text: text.to_string(),
            project_id,
            status,
            details: Some(details.to_string()),
        }
    }

    fn day(tasks: Vec<Task>) -> Day {
        Day {
            date_time: NaiveDate::from_ymd_opt(2026, 11, 9).unwrap(),
            tasks,
        }
    }

    #[test]
    fn writes_the_canonical_form() {
        let day = day(vec![
            task(3, "Pay the rent", 2, TaskStatus::Done, ""),
            task(1, "Buy eggs", 0, TaskStatus::ToDo, ""),
            task(0, "", 0, TaskStatus::ToDo, ""),
            task(
                2,
                "Read \"The Mythical Man-Month\"",
                0,
                TaskStatus::ToDo,
                "Chapters 1 and 2.\nThe rest, next week.",
            ),
            task(4, " ", 2, TaskStatus::Dropped, "\n"),
        ]);
        assert_eq!(
            day_to_string(&day),
            r#"date_time = "2026-11-09"

[[tasks]]
id = "00000000-0000-0000-0000-000000000001"
text = "Buy eggs"
project_id = 0
status = "ToDo"

[[tasks]]
id = "00000000-0000-0000-0000-000000000002"
text = "Read \"The Mythical Man-Month\""
project_id = 0
status = "ToDo"
details = """
Chapters 1 and 2.
The rest, next week."""

[[tasks]]
id = "00000000-0000-0000-0000-000000000003"
text = "Pay the rent"
project_id = 2
status = "Done"
"#
        );
    }

    #[test]
    fn sorts_by_status_then_project_keeping_the_order_within_a_project() {
        let day = day(vec![
            task(1, "b", 1, TaskStatus::Doing, ""),
            task(2, "z", 0, TaskStatus::ToDo, ""),
            task(3, "a", 1, TaskStatus::Doing, ""),
            task(4, "y", 0, TaskStatus::ToDo, ""),
            task(5, "c", 0, TaskStatus::Doing, ""),
        ]);
        let written: Day = toml::from_str(&day_to_string(&day)).unwrap();
        let titles: Vec<_> = written.tasks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(titles, ["z", "y", "c", "b", "a"]);
    }

    #[test]
    fn escapes() {
        assert_eq!(
            escape_single_line("a\\b\"c\nd\te\rf\u{1}g\u{7F}é"),
            r#"a\\b\"c\nd\te\rf\u0001g\u007Fé"#
        );
        assert_eq!(escape_multi_line("a\\b\nc\td\re\u{1}"), "a\\\\b\nc\td\\re\\u0001");
        assert_eq!(escape_multi_line(r#"say "hi""#), r#"say "hi""#);
        // Five quotes: the third one is escaped, and the count starts again.
        assert_eq!(escape_multi_line(&"\"".repeat(5)), "\"\"\\\"\"\"");
        assert_eq!(escape_multi_line("a\"\"\"b"), "a\"\"\\\"b");
    }

    /// Whatever the text, reading the file gives back the same text.
    #[test]
    fn round_trips() {
        let texts = [
            "",
            "plain",
            "ünïcödé ≡ 🦀",
            "\"",
            "\"\"",
            "\"\"\"",
            "\"\"\"\"\"\"\"",
            "ends with a quote\"",
            "ends with two quotes\"\"",
            "ends with three quotes\"\"\"",
            "\"starts with a quote",
            "back\\slash",
            "ends with a backslash\\",
            "line\\\ncontinuation?",
            "\nstarts with a new line",
            "ends with a new line\n",
            "\n\n",
            "tab\there",
            "crlf\r\nline",
            "control \u{0} \u{1B} \u{7F}",
            "'''literal'''",
            "\"\"\"\n\"\"\"",
        ];
        for (i, text) in texts.iter().enumerate() {
            let mut original = task(i as u128 + 1, text, 0, TaskStatus::ToDo, text);
            if text.trim().is_empty() {
                // Would be blank, and not written.
                original.text = format!("x{text}");
            }
            let written = day_to_string(&day(vec![original.clone()]));
            let read: Day = toml::from_str(&written).unwrap_or_else(|err| panic!("{text:?}: {err}\n{written}"));
            let expected_details = (!text.is_empty()).then(|| text.to_string());
            assert_eq!(read.tasks[0].text, original.text, "{written}");
            assert_eq!(read.tasks[0].details, expected_details, "{written}");
            assert_eq!(read.tasks[0].id, original.id);
        }
    }
}
