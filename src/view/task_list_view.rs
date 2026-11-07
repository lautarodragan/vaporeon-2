//! A [`TaskList`] as GTK widgets: a scrollable column of rows (status headers, project headers, tasks).
//!
//! The model (`TaskList`) is the source of truth. [`TaskListView::update`] makes the widgets match it:
//! when the structure changes (tasks added, removed or moved, another day loaded, a different filter...) the rows
//! are rebuilt; otherwise the existing rows are updated in place (texts, selection, editing).

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use gtk::{gdk, glib, prelude::*};

use crate::{
    components::{EditableTask, task_list::TaskList},
    theme::DetailsIndicator,
    view::fade_edges::FadeEdges,
};

/// Called when a task is clicked, with the indices of its status, project and task, and the number of clicks
/// (2 for a double click).
pub type OnTaskClick = Rc<dyn Fn(usize, usize, usize, i32)>;
/// Called with the new text, every time the text of the task being edited changes.
pub type OnEdit = Rc<dyn Fn(String)>;

/// Drag and drop of tasks.
pub struct DragAndDrop {
    /// Called when starting to drag a task (status, project, task). Returns the text to drag (its title),
    /// or None if the task can't be dragged.
    pub prepare: Rc<dyn Fn(usize, usize, usize) -> Option<String>>,
    /// Called when text is dropped on a row (status, project, position between rows, text).
    /// The text is either a task being moved, or comes from another application.
    pub drop: Rc<dyn Fn(usize, usize, usize, String) -> bool>,
    /// Called when a drag ends, whether it was dropped or not.
    pub end: Rc<dyn Fn()>,
    /// Whether dropping at this position (status, project, position between rows) would leave everything as it is,
    /// i.e. it's where the dragged task already is. No drop indicator is shown there.
    pub is_noop: Rc<dyn Fn(usize, usize, usize) -> bool>,
}

/// How many rows to keep visible above and below the selected task, when moving the selection with the keyboard.
const SCROLL_PADDING_ROWS: f64 = 5.0;

/// The parts of the model that, when they change, require rebuilding the rows.
#[derive(Debug, PartialEq, Eq)]
struct Structure {
    filter: Option<usize>,
    /// For each status: for each project, (name, whether its title is shown, number of tasks).
    statuses: Vec<Vec<(String, bool, usize)>>,
}

impl Structure {
    fn of(list: &TaskList) -> Self {
        Self {
            filter: list.project_filter(),
            statuses: list
                .status_lists()
                .iter()
                .map(|status| {
                    status
                        .tasks_by_project
                        .iter()
                        .map(|p| (p.project_name.clone(), p.show_title, p.tasks.len()))
                        .collect()
                })
                .collect(),
        }
    }
}

/// How tasks with details (a description) look. See `details_indicator` in the theme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailsStyle {
    pub indicator: DetailsIndicator,
    pub bullet: String,
    pub bullet_with_details: String,
    pub marker: String,
}

/// A task: `[gutter] [bullet] [title or editor] [preview] [marker]`.
struct TaskRow {
    row: gtk::Box,
    /// Dot in the margin, for tasks with details (`DetailsIndicator::Gutter`).
    gutter: gtk::Box,
    bullet: gtk::Label,
    label: gtk::Label,
    editor: Option<gtk::Text>,
    /// Beginning of the details (`DetailsIndicator::Preview`).
    preview: gtk::Label,
    /// Mark at the end of the row (`DetailsIndicator::Right`).
    marker: gtk::Label,
}

impl TaskRow {
    fn new(status: usize, project: usize, task: usize, on_click: OnTaskClick, dnd: &DragAndDrop) -> Self {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        row.add_css_class("row");
        row.add_css_class("task");

        let gutter = gtk::Box::builder().valign(gtk::Align::Center).build();
        gutter.add_css_class("gutter");

        let bullet = gtk::Label::builder().label("-").xalign(0.0).build();
        bullet.add_css_class("bullet");

        let label = gtk::Label::builder()
            .xalign(0.0)
            .single_line_mode(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        label.add_css_class("title");

        // Takes the rest of the row, so the marker is at its right end.
        let preview = gtk::Label::builder()
            .xalign(0.0)
            .hexpand(true)
            .single_line_mode(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        preview.add_css_class("preview");

        let marker = gtk::Label::builder().xalign(1.0).visible(false).build();
        marker.add_css_class("marker");

        row.append(&gutter);
        row.append(&bullet);
        row.append(&label);
        row.append(&preview);
        row.append(&marker);

        let click = gtk::GestureClick::new();
        click.connect_pressed(move |gesture, n_press, _, _| {
            // So the panel's own click handler doesn't run too.
            gesture.set_state(gtk::EventSequenceState::Claimed);
            on_click(status, project, task, n_press);
        });
        row.add_controller(click.clone());

        // Dragging this task.
        let drag = gtk::DragSource::builder().actions(gdk::DragAction::MOVE).build();
        let hotspot = Rc::new(Cell::new((0, 0)));
        drag.connect_prepare(glib::clone!(
            #[strong(rename_to = prepare)]
            dnd.prepare,
            #[strong]
            hotspot,
            move |_, x, y| {
                let text = prepare(status, project, task)?;
                hotspot.set((x as i32, y as i32));
                Some(gdk::ContentProvider::for_value(&text.to_value()))
            }
        ));
        drag.connect_drag_begin(glib::clone!(
            #[weak]
            row,
            move |drag, _| {
                // The row itself is what's shown under the pointer while dragging. A snapshot of it, not the live
                // widget: otherwise, when dragging over the row itself, the drop indicator would show up on the
                // icon too.
                let (x, y) = hotspot.get();
                let snapshot = gtk::WidgetPaintable::new(Some(&row)).current_image();
                drag.set_icon(Some(&snapshot), x, y);
            }
        ));
        drag.connect_drag_end(glib::clone!(
            #[strong(rename_to = end)]
            dnd.end,
            move |_, _, _| end()
        ));
        row.add_controller(drag.clone());
        // Both gestures work on the same clicks: the click selects the task, then dragging moves it.
        click.group_with(&drag);

        // Dropping on this task: above it if dropped on its top half, below it otherwise.
        let target = gtk::DropTarget::new(glib::Type::STRING, gdk::DragAction::MOVE | gdk::DragAction::COPY);
        target.connect_motion(glib::clone!(
            #[weak]
            row,
            #[strong(rename_to = is_noop)]
            dnd.is_noop,
            #[upgrade_or]
            gdk::DragAction::empty(),
            move |target, _, y| {
                let above = y < row.height() as f64 / 2.0;
                let position = if above { task } else { task + 1 };
                let indicator = (!is_noop(status, project, position)).then_some(above);
                show_drop_indicator(&row, indicator);
                // Tasks dragged within the app are moved. Text from other apps is copied.
                let actions = target
                    .current_drop()
                    .map(|d| d.actions())
                    .unwrap_or(gdk::DragAction::MOVE);
                if actions.contains(gdk::DragAction::MOVE) {
                    gdk::DragAction::MOVE
                } else {
                    gdk::DragAction::COPY
                }
            }
        ));
        target.connect_leave(glib::clone!(
            #[weak]
            row,
            move |_| show_drop_indicator(&row, None)
        ));
        target.connect_drop(glib::clone!(
            #[weak]
            row,
            #[strong(rename_to = drop)]
            dnd.drop,
            #[upgrade_or]
            false,
            move |_, value, _, y| {
                show_drop_indicator(&row, None);
                let Ok(text) = value.get::<String>() else { return false };
                let before = y < row.height() as f64 / 2.0;
                drop(status, project, if before { task } else { task + 1 }, text)
            }
        ));
        row.add_controller(target);

        Self {
            row,
            label,
            editor: None,
            gutter,
            bullet,
            preview,
            marker,
        }
    }

    fn update(
        &mut self,
        task: &EditableTask,
        selected: bool,
        details: &DetailsStyle,
        on_edit: &OnEdit,
        syncing: &Rc<Cell<bool>>,
    ) {
        let description = task.description().trim();
        let has_details = !description.is_empty();
        let indicator = has_details.then_some(details.indicator);

        let bullet = if indicator == Some(DetailsIndicator::Bullet) {
            &details.bullet_with_details
        } else {
            &details.bullet
        };
        if self.bullet.text() != bullet.as_str() {
            self.bullet.set_text(bullet);
        }
        set_class(&self.bullet, "details", indicator == Some(DetailsIndicator::Bullet));

        set_class(&self.gutter, "details", indicator == Some(DetailsIndicator::Gutter));

        // Only the first line of the details.
        let preview = match indicator {
            Some(DetailsIndicator::Preview) => description.lines().next().unwrap_or_default(),
            _ => "",
        };
        if self.preview.text() != preview {
            self.preview.set_text(preview);
        }

        // While editing, the editor gets the whole row.
        let editing = task.is_editing();
        self.preview.set_visible(!editing);
        self.marker
            .set_visible(indicator == Some(DetailsIndicator::Right) && !editing);
        if self.marker.text() != details.marker.as_str() {
            self.marker.set_text(&details.marker);
        }

        if editing {
            if self.editor.is_none() {
                let editor = gtk::Text::builder().hexpand(true).valign(gtk::Align::Center).build();
                editor.add_css_class("title");

                syncing.set(true);
                editor.set_text(task.title());
                syncing.set(false);

                editor.connect_changed(glib::clone!(
                    #[strong]
                    on_edit,
                    #[strong]
                    syncing,
                    move |editor| {
                        if !syncing.get() {
                            on_edit(editor.text().to_string());
                        }
                    }
                ));

                self.row.remove(&self.label);
                self.row.insert_child_after(&editor, Some(&self.bullet));
                editor.grab_focus_without_selecting();
                editor.set_position(-1);
                self.editor = Some(editor);
            }
            return;
        }

        if let Some(editor) = self.editor.take() {
            self.row.remove(&editor);
            self.row.insert_child_after(&self.label, Some(&self.bullet));
        }

        // An empty task still shows something to underline when selected.
        let text = if selected && task.title().is_empty() {
            " "
        } else {
            task.title()
        };
        if self.label.text() != text {
            self.label.set_text(text);
        }

        if selected {
            self.row.add_css_class("selected");
        } else {
            self.row.remove_css_class("selected");
        }
    }
}

pub struct TaskListView {
    /// The scrolled list, with its edges faded out.
    pub widget: FadeEdges,
    scrolled: gtk::ScrolledWindow,
    content: gtk::Box,
    /// Task rows, by status, project and task.
    rows: Vec<Vec<Vec<TaskRow>>>,
    structure: Option<Structure>,
    last_cursor_y: Option<usize>,
    /// Scroll target (top and bottom of the selected row, in pixels) that couldn't be reached yet, because the
    /// rows were just added and GTK hasn't measured them yet. Applied when the scroll range changes.
    pending_scroll: Rc<Cell<Option<(f64, f64)>>>,
    row_height: Rc<Cell<f64>>,
    on_task_click: OnTaskClick,
    on_edit: OnEdit,
    dnd: DragAndDrop,
    syncing: Rc<Cell<bool>>,
}

impl TaskListView {
    pub fn new(on_task_click: OnTaskClick, on_edit: OnEdit, dnd: DragAndDrop, syncing: Rc<Cell<bool>>) -> Self {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .child(&content)
            .build();
        super::scrollbars::hide(&scrolled);

        let pending_scroll: Rc<Cell<Option<(f64, f64)>>> = Rc::new(Cell::new(None));
        let row_height = Rc::new(Cell::new(1.0));

        scrolled.vadjustment().connect_changed(glib::clone!(
            #[strong]
            pending_scroll,
            #[strong]
            row_height,
            move |adjustment| {
                if let Some((top, bottom)) = pending_scroll.get()
                    && scroll_into_view(adjustment, top, bottom, row_height.get())
                {
                    pending_scroll.set(None);
                }
            }
        ));

        setup_drag_autoscroll(&scrolled, row_height.clone());

        Self {
            widget: FadeEdges::new(&scrolled),
            scrolled,
            content,
            rows: vec![],
            structure: None,
            last_cursor_y: None,
            pending_scroll,
            row_height,
            on_task_click,
            on_edit,
            dnd,
            syncing,
        }
    }

    /// Makes the widgets match the model.
    ///
    /// With `scroll`, if the selection moved, scrolls so the selected task is visible. This is used for keyboard
    /// navigation. Mouse clicks don't scroll (the clicked task is already visible).
    pub fn update(&mut self, list: &TaskList, has_focus: bool, scroll: bool, row_height: f64, details: &DetailsStyle) {
        self.row_height.set(row_height);
        self.widget.set_size(row_height);

        let structure = Structure::of(list);
        let rebuilt = self.structure.as_ref() != Some(&structure);
        if rebuilt {
            self.rebuild(list);
            self.structure = Some(structure);
        }

        for (s, status) in list.status_lists().iter().enumerate() {
            for (p, project) in status.tasks_by_project.iter().enumerate() {
                let Some(rows) = self.rows.get_mut(s).and_then(|r| r.get_mut(p)) else {
                    continue;
                };
                for (t, (task, row)) in project.tasks.iter().zip(rows.iter_mut()).enumerate() {
                    let selected = has_focus
                        && s == list.selected_status_index()
                        && p == status.selected_project
                        && t == project.selected_task;
                    row.update(task, selected, details, &self.on_edit, &self.syncing);
                }
            }
        }

        let cursor_y = list.cursor_y();
        if scroll && (rebuilt || self.last_cursor_y != Some(cursor_y)) {
            let top = cursor_y as f64 * row_height;
            let bottom = top + row_height;
            // Stop any kinetic scrolling still going on (e.g. from the mouse wheel or a touchpad), otherwise it
            // would override the scroll position we set. Disabling kinetic scrolling cancels it.
            self.scrolled.set_kinetic_scrolling(false);
            self.scrolled.set_kinetic_scrolling(true);

            let adjustment = self.scrolled.vadjustment();
            if scroll_into_view(&adjustment, top, bottom, row_height) {
                self.pending_scroll.set(None);
            } else {
                self.pending_scroll.set(Some((top, bottom)));
            }
        }
        self.last_cursor_y = Some(cursor_y);
    }

    fn rebuild(&mut self, list: &TaskList) {
        while let Some(child) = self.content.first_child() {
            self.content.remove(&child);
        }
        self.rows.clear();

        let filter = list.project_filter();

        for (s, status) in list.status_lists().iter().enumerate() {
            self.content.append(&header_row(status.status.as_str()));

            let mut status_rows = vec![];
            let project_count = status.tasks_by_project.len();

            for (p, project) in status.tasks_by_project.iter().enumerate() {
                if filter.is_some_and(|f| f != p) {
                    status_rows.push(vec![]);
                    continue;
                }

                let project_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
                project_box.add_css_class("project");
                let draw_line = filter.is_none();
                if draw_line {
                    project_box.add_css_class("line");
                }

                if project.show_title {
                    project_box.append(&header_row(&project.project_name));
                }

                let tasks_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
                tasks_box.add_css_class("tasks");
                if project.show_title {
                    tasks_box.add_css_class("indented");
                }

                let mut project_rows = vec![];
                for t in 0..project.tasks.len() {
                    let row = TaskRow::new(s, p, t, self.on_task_click.clone(), &self.dnd);
                    tasks_box.append(&row.row);
                    project_rows.push(row);
                }
                project_box.append(&tasks_box);

                // An empty row after each project. The line on the left goes through it,
                // except for the last project.
                if draw_line && p + 1 < project_count {
                    project_box.append(&empty_row());
                    self.content.append(&project_box);
                } else {
                    self.content.append(&project_box);
                    self.content.append(&empty_row());
                }

                status_rows.push(project_rows);
            }

            self.rows.push(status_rows);
        }
    }
}

fn header_row(text: &str) -> gtk::Label {
    let label = gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .single_line_mode(true)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .build();
    label.add_css_class("row");
    label.add_css_class("header");
    label
}

/// While dragging a task near the top or bottom edge of the list, scrolls it, so tasks can be dragged to places
/// that aren't visible.
fn setup_drag_autoscroll(scrolled: &gtk::ScrolledWindow, row_height: Rc<Cell<f64>>) {
    // Pixels to scroll per frame. Negative scrolls up.
    let speed = Rc::new(Cell::new(0.0));
    let timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));

    let stop = glib::clone!(
        #[strong]
        speed,
        #[strong]
        timer,
        move || {
            speed.set(0.0);
            if let Some(timer) = timer.borrow_mut().take() {
                timer.remove();
            }
        }
    );

    let motion = gtk::DropControllerMotion::new();
    motion.connect_motion(glib::clone!(
        #[weak]
        scrolled,
        #[strong]
        stop,
        move |_, _, y| {
            let height = scrolled.height() as f64;
            // Within 2 rows of an edge, scroll. The closer to the edge, the faster.
            let edge = (row_height.get() * 2.0).min(height / 4.0);
            let max_speed = row_height.get() / 2.0;
            let new_speed = if y < edge {
                -max_speed * (edge - y) / edge
            } else if y > height - edge {
                max_speed * (y - (height - edge)) / edge
            } else {
                0.0
            };

            if new_speed == 0.0 {
                stop();
                return;
            }
            speed.set(new_speed);

            if timer.borrow().is_some() {
                return;
            }
            let id = glib::timeout_add_local(
                std::time::Duration::from_millis(16),
                glib::clone!(
                    #[weak]
                    scrolled,
                    #[strong]
                    speed,
                    #[strong]
                    timer,
                    #[upgrade_or_else]
                    || {
                        timer.borrow_mut().take();
                        glib::ControlFlow::Break
                    },
                    move || {
                        let adjustment = scrolled.vadjustment();
                        adjustment.set_value(adjustment.value() + speed.get());
                        glib::ControlFlow::Continue
                    }
                ),
            );
            *timer.borrow_mut() = Some(id);
        }
    ));
    motion.connect_leave(move |_| stop());
    scrolled.add_controller(motion);
}

fn set_class(widget: &impl IsA<gtk::Widget>, class: &str, enabled: bool) {
    if enabled {
        widget.add_css_class(class);
    } else {
        widget.remove_css_class(class);
    }
}

/// Shows a line above (`Some(true)`) or below (`Some(false)`) a row, where a dragged task would be dropped.
fn show_drop_indicator(row: &gtk::Box, above: Option<bool>) {
    row.remove_css_class("drop-above");
    row.remove_css_class("drop-below");
    match above {
        Some(true) => row.add_css_class("drop-above"),
        Some(false) => row.add_css_class("drop-below"),
        None => {}
    }
}

fn empty_row() -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.add_css_class("row");
    row
}

/// Scrolls so that the range `[top, bottom]` is visible, with some padding around it.
/// Returns false if the range is beyond what can currently be scrolled to (e.g. rows that GTK hasn't measured yet).
fn scroll_into_view(adjustment: &gtk::Adjustment, top: f64, bottom: f64, row_height: f64) -> bool {
    let page = adjustment.page_size();
    if page <= 0.0 {
        return false;
    }

    let padding = (SCROLL_PADDING_ROWS * row_height).min(((page - (bottom - top)) / 2.0).max(0.0));
    let mut value = adjustment.value();
    if top - padding < value {
        value = top - padding;
    } else if bottom + padding > value + page {
        value = bottom + padding - page;
    }

    let max = (adjustment.upper() - page).max(adjustment.lower());
    adjustment.set_value(value.clamp(adjustment.lower(), max));
    value <= max + 0.5
}
