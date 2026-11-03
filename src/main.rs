#![allow(clippy::new_without_default)]

use std::{cell::RefCell, error::Error, rc::Rc};

use chrono::Local;
use flexi_logger::{FileSpec, WriteMode};
use gtk::{gdk, gio, glib, prelude::*};

use crate::{
    action::{Action, Actions, ApplicationAction, KeyCode, KeyEvent, OnActions, TextAction},
    canvas::{Canvas, EditTarget},
    components::root::Root,
    editors::Editors,
    log_format::log_format,
    settings::Settings,
    theme::Theme,
    toml_io::get_config_file_path,
};

mod action;
mod canvas;
mod clipboard;
mod components;
mod editors;
mod git;
mod log_format;
mod models;
mod settings;
mod theme;
mod toml_io;

const APP_ID: &str = "codes.taro.Vaporeon";

/// Makes the window itself fully transparent. The background color (and its opacity) is painted by us,
/// in [`Root::render`], so it can be configured in the theme.
const CSS: &str = "
window.vaporeon,
window.vaporeon.background,
window.vaporeon > * {
    background: none;
    background-color: transparent;
    box-shadow: none;
}
";

pub struct App {
    root: Root,
    settings: Settings,
    theme: Theme,
    actions: Actions,
    /// Kept around because the file monitor stops working when dropped.
    theme_file_monitor: Option<gio::FileMonitor>,
}

pub type AppRef = Rc<RefCell<App>>;

fn main() -> Result<(), Box<dyn Error>> {
    let _logger = flexi_logger::Logger::try_with_str("vaporeon=trace, warn")?
        .format(log_format)
        .log_to_file(FileSpec::default().suppress_timestamp())
        .write_mode(WriteMode::Direct)
        .use_utc()
        .start()?;

    log::trace!("Opening app...");

    let app = gtk::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);

    // Our own flags (`--log-events`, `--log-target`) are read with std::env::args, so we don't let GTK see them,
    // otherwise it'd complain about unknown options.
    let program_name = std::env::args().next().unwrap_or_else(|| "vaporeon".to_string());
    app.run_with_args(&[program_name]);

    Ok(())
}

fn build_ui(application: &gtk::Application) {
    // Activating an already-running instance (e.g. launching it twice) just focuses the existing window.
    if let Some(window) = application.active_window() {
        window.present();
        return;
    }

    let actions = Actions::from_file_or_default();
    assert!(
        actions.contains(Action::Application(ApplicationAction::Quit)),
        "No key binding for Action::Quit! User would not be able to exit Vaporeon. This is 100% a bug."
    );

    let settings = Settings::load();
    let theme = Theme::load();

    let mut root = Root::new(settings.clone());
    root.load_selected_day();
    root.load_global();

    let app: AppRef = Rc::new(RefCell::new(App {
        root,
        settings,
        theme,
        actions,
        theme_file_monitor: None,
    }));

    load_css();

    let window = gtk::ApplicationWindow::builder()
        .application(application)
        .title("Vaporeon")
        .default_width(1200)
        .default_height(800)
        .decorated(app.borrow().theme.window_decorations)
        .build();
    window.add_css_class("vaporeon");

    let drawing_area = gtk::DrawingArea::builder()
        .hexpand(true)
        .vexpand(true)
        .focusable(true)
        .can_focus(true)
        .build();

    let editors = Rc::new(Editors::new(&drawing_area));
    editors.apply_theme(&app.borrow().theme);
    editors.connect(&app);

    // The drawing area is redrawn (entirely) only when something changes: after handling a key press, when the
    // window is resized, when the theme changes, etc. See `queue_draw` calls.
    drawing_area.set_draw_func({
        let app = app.clone();
        let editors = editors.clone();
        move |area, cr, width, height| {
            let editor_placement = {
                let mut app = app.borrow_mut();
                let App { root, theme, .. } = &mut *app;
                let canvas = Canvas::new(cr, &area.pango_context(), theme, width as f64, height as f64);
                root.set_visible_rows(Root::visible_rows(&canvas));
                root.refresh_scroll();
                root.render(&canvas);
                canvas.editor()
            };
            editors.after_draw(&app, editor_placement);
        }
    });

    setup_input(&app, &window, &editors);
    watch_theme_file(&app, &window, &editors);

    window.connect_close_request({
        let app = app.clone();
        move |_| {
            let app = app.borrow();
            app.root.save_global();
            app.settings.save();
            glib::Propagation::Proceed
        }
    });

    // The week view highlights "today", so redraw when the day changes.
    glib::timeout_add_seconds_local(30, {
        let drawing_area = drawing_area.downgrade();
        let mut today = Local::now().date_naive();
        move || {
            let Some(drawing_area) = drawing_area.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let now = Local::now().date_naive();
            if now != today {
                today = now;
                drawing_area.queue_draw();
            }
            glib::ControlFlow::Continue
        }
    });

    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&drawing_area));
    overlay.add_overlay(&editors.fixed);

    window.set_child(Some(&overlay));
    window.present();
    drawing_area.grab_focus();
}

fn load_css() {
    let Some(display) = gdk::Display::default() else {
        log::error!("No display!");
        return;
    };
    let provider = gtk::CssProvider::new();
    provider.load_from_string(CSS);
    gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
}

/// Whether `action` ends editing `target`.
fn ends_editing(target: EditTarget, action: &Action) -> bool {
    matches!(
        (target, action),
        (
            EditTarget::TaskTitle,
            Action::Text(TextAction::ConfirmEdit | TextAction::CancelEdit)
        ) | (
            EditTarget::TaskDetails,
            Action::Text(TextAction::ConfirmEditMultiLine | TextAction::CancelEdit)
        )
    )
}

fn setup_input(app: &AppRef, window: &gtk::ApplicationWindow, editors: &Rc<Editors>) {
    let key_controller = gtk::EventControllerKey::new();

    // Capture phase: we see key presses before the focused widget (e.g. a text editor) does.
    key_controller.set_propagation_phase(gtk::PropagationPhase::Capture);

    key_controller.connect_key_pressed({
        let app = app.clone();
        let window = window.downgrade();
        let editors = editors.clone();
        move |_, key, _keycode, state| {
            let Some(event) = KeyEvent::from_gdk(key, state) else {
                return glib::Propagation::Proceed;
            };

            if std::env::args().any(|arg| arg == "--log-events") {
                log::debug!("event: {event:?} ({key:?}, {state:?})");
            }

            let (editing, actions) = {
                let app = app.borrow();
                (app.root.editing(), app.actions.actions_by_key(event))
            };

            if actions.contains(&Action::Application(ApplicationAction::Quit)) {
                if let Some(window) = window.upgrade() {
                    window.close();
                }
                return glib::Propagation::Stop;
            }

            if actions.contains(&Action::Application(ApplicationAction::Save)) {
                let app = app.borrow();
                app.root.save();
                app.root.save_global();
                return glib::Propagation::Stop;
            }

            if let Some(target) = editing {
                // While editing, the text editor gets every key, except for the ones that stop editing.
                if let Some(action) = actions.into_iter().find(|a| ends_editing(target, a)) {
                    app.borrow_mut().root.on_actions(vec![action]);
                    editors.hide();
                    editors.drawing_area.queue_draw();
                    return glib::Propagation::Stop;
                }

                // Don't let these move the focus out of a single-line editor.
                if target == EditTarget::TaskTitle
                    && matches!(
                        event.code,
                        KeyCode::Up
                            | KeyCode::Down
                            | KeyCode::PageUp
                            | KeyCode::PageDown
                            | KeyCode::Tab
                            | KeyCode::BackTab
                    )
                {
                    return glib::Propagation::Stop;
                }

                return glib::Propagation::Proceed;
            }

            if actions.contains(&Action::Application(ApplicationAction::Paste)) {
                paste(&app, &editors.drawing_area);
            } else {
                app.borrow_mut().root.on_actions(actions);
            }

            editors.drawing_area.queue_draw();

            glib::Propagation::Stop
        }
    });

    window.add_controller(key_controller);
}

/// Reads text from the clipboard and adds it as a new task.
/// (While editing, pasting is handled by the text editor itself.)
fn paste(app: &AppRef, drawing_area: &gtk::DrawingArea) {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let clipboard = display.clipboard();
    let app = app.clone();
    let drawing_area = drawing_area.downgrade();

    glib::spawn_future_local(async move {
        let text = match clipboard.read_text_future().await {
            Ok(Some(text)) => text.to_string(),
            Ok(None) => {
                log::debug!("Clipboard is empty or doesn't contain text");
                return;
            }
            Err(err) => {
                log::debug!("Clipboard error (but should be fine): {err:?}");
                return;
            }
        };

        log::debug!("Pasted: {text}");

        app.borrow_mut()
            .root
            .on_actions(vec![Action::Application(ApplicationAction::PasteText(text))]);

        if let Some(drawing_area) = drawing_area.upgrade() {
            drawing_area.queue_draw();
        }
    });
}

/// Reloads the theme whenever `~/.config/vaporeon/theme.toml` changes, so it's easy to tweak colors, fonts
/// and opacity while looking at the result.
fn watch_theme_file(app: &AppRef, window: &gtk::ApplicationWindow, editors: &Rc<Editors>) {
    let Ok(path) = get_config_file_path("theme") else {
        return;
    };
    let file = gio::File::for_path(&path);
    let monitor = match file.monitor_file(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) {
        Ok(monitor) => monitor,
        Err(err) => {
            log::warn!("Can't watch {path:?} for changes: {err:?}");
            return;
        }
    };

    monitor.connect_changed({
        let app = app.clone();
        let window = window.downgrade();
        let editors = Rc::downgrade(editors);
        move |_, _, _, event| {
            if !matches!(
                event,
                gio::FileMonitorEvent::ChangesDoneHint
                    | gio::FileMonitorEvent::Created
                    | gio::FileMonitorEvent::Deleted
            ) {
                return;
            }
            log::debug!("Theme file changed. Reloading it.");
            let theme = Theme::load();
            if let Some(window) = window.upgrade() {
                window.set_decorated(theme.window_decorations);
            }
            if let Some(editors) = editors.upgrade() {
                editors.apply_theme(&theme);
                editors.drawing_area.queue_draw();
            }
            app.borrow_mut().theme = theme;
        }
    });

    app.borrow_mut().theme_file_monitor = Some(monitor);
}
