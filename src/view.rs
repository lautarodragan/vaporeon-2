//! The UI, as GTK widgets.
//!
//! ```text
//! window
//! └── main box
//!     ├── header (drawing area: project filter, date, week)
//!     └── panels
//!         ├── day task list
//!         └── right panel: nothing | global task list | task details
//! ```
//!
//! The model ([`Root`](crate::components::root::Root)) is the source of truth. After anything changes it,
//! [`Ui::refresh`] updates the widgets to match.

use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

use gtk::{glib, prelude::*};

use crate::{
    AppRef,
    canvas::{Canvas, Metrics},
    components::root::{HEADER_ROWS, Panel, RightPanel, TaskPosition},
    theme::Theme,
    view::{
        details_view::DetailsView,
        style::WINDOW_PADDING,
        task_list_view::{DragAndDrop, OnEdit, OnTaskClick, TaskListView},
    },
};

mod details_view;
mod style;
mod task_list_view;

/// Below this width (in pixels), only one panel is shown, using the full width.
const TWO_PANELS_MIN_WIDTH: i32 = 900;

pub struct Ui {
    pub window: gtk::ApplicationWindow,
    header: gtk::DrawingArea,
    main_box: gtk::Box,
    right_stack: gtk::Stack,
    day: RefCell<TaskListView>,
    global: RefCell<TaskListView>,
    details: RefCell<DetailsView>,
    css: gtk::CssProvider,
    metrics: Cell<Metrics>,
    refresh_scheduled: Cell<bool>,
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
                    schedule_refresh(&ui, &app);
                })
            };

            let on_task_click = |panel: Panel| -> OnTaskClick {
                let app = app.clone();
                let ui = ui.clone();
                Rc::new(move |status, project, task, n_press| {
                    app.borrow_mut()
                        .root
                        .click_task(panel, status, project, task, n_press >= 2);
                    schedule_refresh(&ui, &app);
                })
            };

            let on_details_click: Rc<dyn Fn()> = {
                let app = app.clone();
                let ui = ui.clone();
                Rc::new(move || {
                    app.borrow_mut().root.click_details();
                    schedule_refresh(&ui, &app);
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
                        schedule_refresh(&ui, &app);
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
            header.set_draw_func({
                let app = app.clone();
                move |area, cr, width, _height| {
                    let app = app.borrow();
                    let canvas = Canvas::new(cr, &area.pango_context(), &app.theme, width as f64);
                    app.root.render_header(&canvas);
                }
            });
            // Whether there's room for both panels depends on the width.
            header.connect_resize({
                let app = app.clone();
                let ui = ui.clone();
                move |_, _, _| schedule_refresh(&ui, &app)
            });

            let main_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .margin_start(WINDOW_PADDING)
                .margin_end(WINDOW_PADDING)
                .build();
            main_box.append(&header);
            main_box.append(&panels);

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
                right_stack,
                day: RefCell::new(day),
                global: RefCell::new(global),
                details: RefCell::new(details),
                css,
                metrics: Cell::new(metrics),
                refresh_scheduled: Cell::new(false),
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
}

/// Refreshes the UI as soon as GTK is idle. Used from signal handlers (clicks, text changes), where the widgets
/// emitting the signal shouldn't be removed or rebuilt in the middle of it.
fn schedule_refresh(ui: &Weak<Ui>, app: &AppRef) {
    let Some(ui) = ui.upgrade() else { return };
    if ui.refresh_scheduled.replace(true) {
        return;
    }
    let app = app.clone();
    glib::idle_add_local_once(move || {
        ui.refresh_scheduled.set(false);
        ui.refresh(&app, false);
    });
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
            schedule_refresh(&ui, &app);
        }
    });
    widget.add_controller(click);
}
