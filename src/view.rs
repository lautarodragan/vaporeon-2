//! The UI, as GTK widgets.
//!
//! ```text
//! window
//! └── main box
//!     ├── header (drawing area: project filter, date, week)
//!     ├── panels
//!     │   ├── day task list
//!     │   └── right panel: nothing | global task list | task details
//!     └── status bar (messages and prompts)
//! ```
//!
//! The model ([`Root`](crate::components::root::Root)) is the source of truth. After anything changes it,
//! [`Ui::refresh`] updates the widgets to match.

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    path::PathBuf,
    rc::{Rc, Weak},
    time::Duration,
};

use gtk::{gio, glib, prelude::*};

use crate::{
    AppRef,
    canvas::{Canvas, Metrics},
    components::root::{
        DataFile,
        ExternalChange,
        HEADER_ROWS,
        HeaderLink,
        Panel,
        Resolution,
        RightPanel,
        TaskPosition,
    },
    theme::Theme,
    view::{
        details_view::DetailsView,
        status_bar::{PromptOption, StatusBar},
        style::WINDOW_PADDING,
        task_list_view::{DragAndDrop, OnEdit, OnTaskClick, TaskListView},
    },
};

mod details_view;
mod status_bar;
mod style;
mod task_list_view;

/// Below this width (in pixels), only one panel is shown, using the full width.
const TWO_PANELS_MIN_WIDTH: i32 = 900;

/// How long to wait after a file changes before checking it. Programs (and git) often write files in several steps.
const FILE_CHANGE_DELAY: Duration = Duration::from_millis(250);

const CONFLICT_OPTIONS: [PromptOption; 3] = [('r', "reload"), ('k', "keep mine"), ('b', "backup mine & reload")];

pub struct Ui {
    pub window: gtk::ApplicationWindow,
    header: gtk::DrawingArea,
    main_box: gtk::Box,
    panels: gtk::Box,
    right_stack: gtk::Stack,
    status_bar: StatusBar,
    day: RefCell<TaskListView>,
    global: RefCell<TaskListView>,
    details: RefCell<DetailsView>,
    css: gtk::CssProvider,
    metrics: Cell<Metrics>,
    refresh_scheduled: Cell<bool>,
    refresh_scroll: Cell<bool>,
    /// The file being watched for changes, for each data file, and its monitor.
    file_monitors: RefCell<HashMap<DataFile, (PathBuf, gio::FileMonitor)>>,
    /// Files waiting to be checked for changes (see [`FILE_CHANGE_DELAY`]).
    file_checks_scheduled: RefCell<Vec<DataFile>>,
    /// Files that changed on disk while having unsaved changes, waiting for the user to choose what to do.
    conflicts: RefCell<VecDeque<DataFile>>,
    app: AppRef,
    this: Weak<Ui>,
}

impl Ui {
    pub fn new(app: &AppRef, window: gtk::ApplicationWindow) -> Rc<Self> {
        Rc::new_cyclic(|ui: &Weak<Ui>| {
            // True while the views set the text of their editors themselves,
            // so the editors' "changed" handlers can ignore it.
            let syncing = Rc::new(Cell::new(false));

            let on_edit: OnEdit = {
                let app = app.clone();
                let ui = ui.clone();
                Rc::new(move |text| {
                    app.borrow_mut().root.set_editing_text(text);
                    schedule_refresh(&ui, &app, false);
                })
            };

            let on_task_click = |panel: Panel| -> OnTaskClick {
                let app = app.clone();
                let ui = ui.clone();
                Rc::new(move |status, project, task, n_press| {
                    app.borrow_mut()
                        .root
                        .click_task(panel, status, project, task, n_press >= 2);
                    schedule_refresh(&ui, &app, false);
                })
            };

            let on_details_click: Rc<dyn Fn()> = {
                let app = app.clone();
                let ui = ui.clone();
                Rc::new(move || {
                    app.borrow_mut().root.click_details();
                    schedule_refresh(&ui, &app, false);
                })
            };

            // The task being dragged, if the drag started in this app.
            let dragged: Rc<Cell<Option<TaskPosition>>> = Rc::new(Cell::new(None));

            let drag_and_drop = |panel: Panel| DragAndDrop {
                prepare: {
                    let app = app.clone();
                    let dragged = dragged.clone();
                    Rc::new(move |status, project, task| {
                        let app = app.borrow();
                        if app.root.editing().is_some() {
                            return None;
                        }
                        let title = app
                            .root
                            .list(panel)?
                            .status_lists()
                            .get(status)?
                            .tasks_by_project
                            .get(project)?
                            .tasks
                            .get(task)
                            .filter(|t| !t.is_empty())?
                            .title()
                            .to_string();
                        dragged.set(Some(TaskPosition {
                            panel,
                            status,
                            project,
                            task,
                        }));
                        Some(title)
                    })
                },
                drop: {
                    let app = app.clone();
                    let ui = ui.clone();
                    let dragged = dragged.clone();
                    Rc::new(move |status, project, task, text| {
                        let to = TaskPosition {
                            panel,
                            status,
                            project,
                            task,
                        };
                        let dropped = match dragged.take() {
                            Some(from) => app.borrow_mut().root.move_task(from, to),
                            None => app.borrow_mut().root.insert_dropped_text(to, &text),
                        };
                        // Not right away: the dragged row may be removed when refreshing.
                        schedule_refresh(&ui, &app, false);
                        dropped
                    })
                },
                end: {
                    let dragged = dragged.clone();
                    Rc::new(move || dragged.set(None))
                },
                is_noop: {
                    let dragged = dragged.clone();
                    Rc::new(move |status, project, position| {
                        dragged.get().is_some_and(|from| {
                            from.panel == panel
                                && from.status == status
                                && from.project == project
                                && (position == from.task || position == from.task + 1)
                        })
                    })
                },
            };

            let day = TaskListView::new(
                on_task_click(Panel::Left),
                on_edit.clone(),
                drag_and_drop(Panel::Left),
                syncing.clone(),
            );
            let global = TaskListView::new(
                on_task_click(Panel::Right),
                on_edit.clone(),
                drag_and_drop(Panel::Right),
                syncing.clone(),
            );
            let details = DetailsView::new(on_edit, on_details_click, syncing);

            let right_stack = gtk::Stack::builder().hexpand(true).vexpand(true).build();
            right_stack.add_named(&gtk::Box::new(gtk::Orientation::Vertical, 0), Some("nothing"));
            right_stack.add_named(&global.scrolled, Some("global"));
            right_stack.add_named(&details.scrolled, Some("details"));

            // Clicking on an empty part of a panel focuses it.
            add_panel_click(&day.scrolled, Panel::Left, app, ui);
            add_panel_click(&right_stack, Panel::Right, app, ui);

            let panels = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .homogeneous(true)
                .vexpand(true)
                .build();
            panels.append(&day.scrolled);
            panels.append(&right_stack);

            let header = gtk::DrawingArea::builder().hexpand(true).focusable(true).build();
            // Clickable parts of the header, as of the last time it was drawn.
            let links: Rc<RefCell<Vec<HeaderLink>>> = Rc::new(RefCell::new(vec![]));
            header.set_draw_func({
                let app = app.clone();
                let links = links.clone();
                move |area, cr, width, _height| {
                    let app = app.borrow();
                    let canvas = Canvas::new(cr, &area.pango_context(), &app.theme, width as f64);
                    *links.borrow_mut() = app.root.render_header(&canvas);
                }
            });
            add_header_links(&header, links, app, ui);
            // Whether there's room for both panels depends on the width.
            header.connect_resize({
                let app = app.clone();
                let ui = ui.clone();
                move |_, _, _| schedule_refresh(&ui, &app, false)
            });

            let main_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .margin_start(WINDOW_PADDING)
                .margin_end(WINDOW_PADDING)
                .build();
            let status_bar = StatusBar::new();

            main_box.append(&header);
            main_box.append(&panels);
            main_box.append(&status_bar.widget);

            window.set_child(Some(&main_box));

            let css = gtk::CssProvider::new();
            gtk::style_context_add_provider_for_display(
                &WidgetExt::display(&window),
                &css,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );

            let metrics = Metrics::new(&header.pango_context(), &app.borrow().theme);

            Ui {
                window,
                header,
                main_box,
                panels,
                right_stack,
                status_bar,
                day: RefCell::new(day),
                global: RefCell::new(global),
                details: RefCell::new(details),
                css,
                metrics: Cell::new(metrics),
                refresh_scheduled: Cell::new(false),
                refresh_scroll: Cell::new(false),
                file_monitors: RefCell::new(HashMap::new()),
                file_checks_scheduled: RefCell::new(vec![]),
                conflicts: RefCell::new(VecDeque::new()),
                app: app.clone(),
                this: ui.clone(),
            }
        })
    }

    /// Applies colors, font, etc. Called on startup and when the theme file changes.
    pub fn apply_theme(&self, theme: &Theme) {
        let metrics = Metrics::new(&self.header.pango_context(), theme);
        self.metrics.set(metrics);
        self.css.load_from_string(&style::css(theme, &metrics));
        self.header
            .set_content_height((metrics.line_height * HEADER_ROWS).round() as i32);
        self.window.set_decorated(theme.window_decorations);
    }

    /// Makes the widgets match the model.
    ///
    /// `scroll`: whether to scroll to the selected task if it moved (for keyboard navigation).
    pub fn refresh(&self, app: &AppRef, scroll: bool) {
        for message in app.borrow_mut().root.take_messages() {
            self.status_bar.show_message(&message.text, message.is_error);
        }

        self.watch_files();

        let app = app.borrow();
        let root = &app.root;
        let metrics = self.metrics.get();
        let focus = root.selected_panel();

        self.day
            .borrow_mut()
            .update(root.task_list_day(), focus == Panel::Left, scroll, metrics.line_height);
        self.global.borrow_mut().update(
            root.task_list_global(),
            focus == Panel::Right,
            scroll,
            metrics.line_height,
        );
        self.details
            .borrow_mut()
            .update(root.task_details(), focus == Panel::Right, &metrics);

        self.right_stack.set_visible_child_name(match root.right_panel() {
            RightPanel::Nothing => "nothing",
            RightPanel::GlobalTaskList => "global",
            RightPanel::TaskDetails => "details",
        });

        let width = match self.main_box.width() {
            0 => self.window.default_width() - 2 * WINDOW_PADDING,
            width => width,
        };
        let single_panel = width < TWO_PANELS_MIN_WIDTH;
        self.day
            .borrow()
            .scrolled
            .set_visible(!single_panel || focus == Panel::Left);
        self.right_stack.set_visible(!single_panel || focus == Panel::Right);

        self.header.queue_draw();

        // When not editing, keep the keyboard focus on the header (it doesn't show any focus indicator),
        // so that key presses reach our key handler instead of a text editor. Some widget must have the focus:
        // otherwise GTK doesn't deliver key presses at all.
        if root.editing().is_none() && !self.header.has_focus() {
            self.header.grab_focus();
        }
    }

    /// Redraws the header. E.g. when the day changes, since it highlights "today".
    pub fn redraw_header(&self) {
        self.header.queue_draw();
    }

    /// Whether a question is waiting for an answer in the status bar. While one is, the rest of the UI doesn't
    /// respond: only the options of the question do.
    pub fn is_prompt_active(&self) -> bool {
        self.status_bar.is_prompt_active()
    }

    /// Answers the question in the status bar with the given key. Returns false if it isn't one of the options.
    pub fn answer_prompt(&self, key: char) -> bool {
        let resolution = match key.to_ascii_lowercase() {
            'r' => Resolution::Reload,
            'k' => Resolution::KeepMine,
            'b' => Resolution::BackupAndReload,
            _ => return false,
        };
        let Some(file) = self.conflicts.borrow_mut().pop_front() else {
            return false;
        };

        self.app.borrow_mut().root.resolve_conflict(file, resolution);
        self.status_bar.hide();
        self.panels.set_can_target(true);
        self.header.set_can_target(true);
        self.refresh(&self.app, true);
        self.show_next_conflict();
        true
    }

    /// Files that changed on disk while having unsaved changes, and for which the user hasn't chosen what to do yet.
    pub fn pending_conflicts(&self) -> Vec<DataFile> {
        self.conflicts.borrow().iter().copied().collect()
    }

    /// Watches the files currently shown (the selected day changes), to notice when they change on disk.
    fn watch_files(&self) {
        for file in DataFile::ALL {
            let path = self.app.borrow().root.file_path(file);
            if self
                .file_monitors
                .borrow()
                .get(&file)
                .is_some_and(|(watched, _)| *watched == path)
            {
                continue;
            }

            // Dropping the previous monitor (if any) stops watching the previous file.
            self.file_monitors.borrow_mut().remove(&file);

            // GIO watches the directory, so this works even if the file doesn't exist yet, or programs replace
            // it (as many editors do when saving) instead of writing to it.
            let monitor =
                match gio::File::for_path(&path).monitor_file(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) {
                    Ok(monitor) => monitor,
                    Err(err) => {
                        log::warn!("Can't watch {path:?} for changes: {err:?}");
                        continue;
                    }
                };
            monitor.connect_changed({
                let ui = self.this.clone();
                move |_, _, _, event| {
                    if matches!(
                        event,
                        gio::FileMonitorEvent::ChangesDoneHint
                            | gio::FileMonitorEvent::Changed
                            | gio::FileMonitorEvent::Created
                            | gio::FileMonitorEvent::Deleted
                    ) && let Some(ui) = ui.upgrade()
                    {
                        ui.schedule_file_check(file);
                    }
                }
            });
            self.file_monitors.borrow_mut().insert(file, (path, monitor));
        }
    }

    fn schedule_file_check(&self, file: DataFile) {
        if self.file_checks_scheduled.borrow().contains(&file) {
            return;
        }
        self.file_checks_scheduled.borrow_mut().push(file);

        let ui = self.this.clone();
        glib::timeout_add_local_once(FILE_CHANGE_DELAY, move || {
            let Some(ui) = ui.upgrade() else { return };
            ui.file_checks_scheduled.borrow_mut().retain(|f| *f != file);
            ui.check_file(file);
        });
    }

    /// Checks whether a file changed on disk. If so: reloads it if there are no unsaved changes, asks otherwise.
    fn check_file(&self, file: DataFile) {
        // Still waiting for an answer about this file: it'll be dealt with then, with whatever is on disk by then.
        if self.conflicts.borrow().contains(&file) {
            return;
        }

        let change = self.app.borrow_mut().root.check_external_change(file);
        match change {
            ExternalChange::None => {}
            ExternalChange::Reloaded => self.refresh(&self.app, true),
            ExternalChange::Conflict => {
                self.conflicts.borrow_mut().push_back(file);
                self.show_next_conflict();
            }
        }
    }

    fn show_next_conflict(&self) {
        if self.status_bar.is_prompt_active() {
            return;
        }
        let Some(file) = self.conflicts.borrow().front().copied() else {
            return;
        };

        let name = self.app.borrow().root.file_display_name(file);
        let text = format!("{name} changed on disk, and you have unsaved changes.");

        // Clicking an option removes the options, so answer once GTK is done handling the click.
        let on_click: Rc<dyn Fn(char)> = {
            let ui = self.this.clone();
            Rc::new(move |key| {
                let ui = ui.clone();
                glib::idle_add_local_once(move || {
                    if let Some(ui) = ui.upgrade() {
                        ui.answer_prompt(key);
                    }
                });
            })
        };

        self.status_bar.show_prompt(&text, &CONFLICT_OPTIONS, on_click);
        // Nothing else responds to the mouse until the question is answered.
        self.panels.set_can_target(false);
        self.header.set_can_target(false);
    }
}

/// Refreshes the UI as soon as GTK is idle. Used from signal handlers (clicks, text changes), where the widgets
/// emitting the signal shouldn't be removed or rebuilt in the middle of it.
///
/// `scroll`: see [`Ui::refresh`].
fn schedule_refresh(ui: &Weak<Ui>, app: &AppRef, scroll: bool) {
    let Some(ui) = ui.upgrade() else { return };
    if scroll {
        ui.refresh_scroll.set(true);
    }
    if ui.refresh_scheduled.replace(true) {
        return;
    }
    let app = app.clone();
    glib::idle_add_local_once(move || {
        ui.refresh_scheduled.set(false);
        ui.refresh(&app, ui.refresh_scroll.replace(false));
    });
}

/// Clicking a day in the header goes to that day. The pointer becomes a hand over them.
fn add_header_links(header: &gtk::DrawingArea, links: Rc<RefCell<Vec<HeaderLink>>>, app: &AppRef, ui: &Weak<Ui>) {
    let link_at = {
        let links = links.clone();
        move |x: f64| {
            links
                .borrow()
                .iter()
                .find(|link| x >= link.x.0 && x < link.x.1)
                .copied()
        }
    };

    let click = gtk::GestureClick::new();
    click.connect_pressed({
        let app = app.clone();
        let ui = ui.clone();
        let link_at = link_at.clone();
        move |_, _, x, _| {
            let Some(link) = link_at(x) else { return };
            {
                let mut app = app.borrow_mut();
                app.root.finish_editing();
                if app.root.selected_date() != link.date {
                    app.root.go_to_date(link.date);
                }
            }
            schedule_refresh(&ui, &app, true);
        }
    });
    header.add_controller(click);

    let motion = gtk::EventControllerMotion::new();
    motion.connect_motion({
        let header = header.downgrade();
        move |_, x, _| {
            let Some(header) = header.upgrade() else { return };
            let cursor = link_at(x).map(|_| "pointer");
            header.set_cursor_from_name(cursor);
        }
    });
    header.add_controller(motion);
}

fn add_panel_click(widget: &impl IsA<gtk::Widget>, panel: Panel, app: &AppRef, ui: &Weak<Ui>) {
    let click = gtk::GestureClick::new();
    click.connect_pressed({
        let app = app.clone();
        let ui = ui.clone();
        move |_, _, _, _| {
            {
                let mut app = app.borrow_mut();
                app.root.finish_editing();
                app.root.focus_panel(panel);
            }
            schedule_refresh(&ui, &app, false);
        }
    });
    widget.add_controller(click);
}
