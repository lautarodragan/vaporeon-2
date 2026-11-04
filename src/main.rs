#![allow(clippy::new_without_default)]

use std::{cell::RefCell, error::Error, rc::Rc};

use chrono::Local;
use flexi_logger::{FileSpec, WriteMode};
use gtk::{gdk, gio, glib, prelude::*};

use crate::{
    action::{Action, Actions, ApplicationAction, KeyCode, KeyEvent, KeyModifiers, OnActions, TextAction},
    components::root::{DataFile, EditTarget, Root},
    log_format::log_format,
    settings::Settings,
    theme::Theme,
    toml_io::{get_config_file_path, get_state_dir_path},
    view::Ui,
};

mod action;
mod canvas;
mod clipboard;
mod components;
mod git;
mod log_format;
mod models;
mod settings;
mod theme;
mod toml_io;
mod view;

const APP_ID: &str = "codes.taro.Vaporeon";

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
    // Logs go to $XDG_STATE_HOME/vaporeon/vaporeon.log (~/.local/state/vaporeon/vaporeon.log by default),
    // or to the current directory if that can't be created.
    let mut log_file = FileSpec::default().basename("vaporeon").suppress_timestamp();
    if let Some(state_dir) = get_state_dir_path()
        && std::fs::create_dir_all(&state_dir).is_ok()
    {
        log_file = log_file.directory(state_dir);
    }

    let _logger = flexi_logger::Logger::try_with_str("vaporeon=trace, warn")?
        .format(log_format)
        .log_to_file(log_file)
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

    let window = gtk::ApplicationWindow::builder()
        .application(application)
        .title("Vaporeon")
        .default_width(1200)
        .default_height(800)
        .build();
    window.add_css_class("vaporeon");

    let ui = Ui::new(&app, window.clone());
    ui.apply_theme(&app.borrow().theme);

    setup_input(&app, &ui);
    watch_theme_file(&app, &ui);

    window.connect_close_request({
        let app = app.clone();
        let ui = ui.clone();
        move |_| {
            let conflicts = ui.pending_conflicts();
            let mut app = app.borrow_mut();
            // Whatever is being edited is kept, as if confirmed.
            app.root.finish_editing();
            // Global first, so it's included in the commit made when saving the day.
            for file in [DataFile::Global, DataFile::Day] {
                if conflicts.contains(&file) {
                    // It changed on disk and the user didn't choose what to do yet: don't overwrite it.
                    // Their version goes to a backup instead.
                    if let Err(err) = app.root.backup(file) {
                        log::error!("Couldn't save a backup of {file:?}: {err:?}");
                    }
                } else {
                    match file {
                        DataFile::Global => app.root.save_global(),
                        DataFile::Day => app.root.save(),
                    };
                }
            }
            app.settings.save();
            glib::Propagation::Proceed
        }
    });

    // The week view highlights "today", so redraw it when the day changes.
    glib::timeout_add_seconds_local(30, {
        let ui = Rc::downgrade(&ui);
        let mut today = Local::now().date_naive();
        move || {
            let Some(ui) = ui.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let now = Local::now().date_naive();
            if now != today {
                today = now;
                ui.redraw_header();
            }
            glib::ControlFlow::Continue
        }
    });

    window.present();
    // After presenting, so the keyboard focus can be set.
    ui.refresh(&app, true);
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

fn setup_input(app: &AppRef, ui: &Rc<Ui>) {
    let key_controller = gtk::EventControllerKey::new();

    // Capture phase: we see key presses before the focused widget (e.g. a text editor) does.
    key_controller.set_propagation_phase(gtk::PropagationPhase::Capture);

    // This handler holds the only strong reference to `ui` (everything else holds weak ones),
    // so the UI lives as long as the window does.
    key_controller.connect_key_pressed({
        let app = app.clone();
        let ui = ui.clone();
        move |_, key, _keycode, state| {
            let Some(event) = KeyEvent::from_gdk(key, state) else {
                return glib::Propagation::Proceed;
            };

            if std::env::args().any(|arg| arg == "--log-events") {
                log::debug!("event: {event:?} ({key:?}, {state:?})");
            }

            // While a question is shown in the status bar, only its options respond.
            if ui.is_prompt_active() {
                if let KeyCode::Char(key) = event.code
                    && !event.modifiers.contains(KeyModifiers::CONTROL)
                    && !event.modifiers.contains(KeyModifiers::ALT)
                {
                    ui.answer_prompt(key);
                }
                return glib::Propagation::Stop;
            }

            let (editing, actions) = {
                let app = app.borrow();
                (app.root.editing(), app.actions.actions_by_key(event))
            };

            if actions.contains(&Action::Application(ApplicationAction::Quit)) {
                ui.window.close();
                return glib::Propagation::Stop;
            }

            if actions.contains(&Action::Application(ApplicationAction::Save)) {
                {
                    let mut app = app.borrow_mut();
                    // Global first, so it's included in the commit made when saving the day.
                    let saved = app.root.save_global() & app.root.save();
                    if saved {
                        let date = app.root.selected_date();
                        app.root.info(format!("Saved {date}."));
                    }
                }
                ui.refresh(&app, false);
                return glib::Propagation::Stop;
            }

            if let Some(target) = editing {
                // While editing, the text editor gets every key, except for the ones that stop editing.
                if let Some(action) = actions.into_iter().find(|a| ends_editing(target, a)) {
                    app.borrow_mut().root.on_actions(vec![action]);
                    ui.refresh(&app, true);
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
                paste(&app, &ui);
            } else {
                app.borrow_mut().root.on_actions(actions);
                ui.refresh(&app, true);
            }

            glib::Propagation::Stop
        }
    });

    ui.window.add_controller(key_controller);
}

/// Reads text from the clipboard and adds it as a new task.
/// (While editing, pasting is handled by the text editor itself.)
fn paste(app: &AppRef, ui: &Rc<Ui>) {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let clipboard = display.clipboard();
    let app = app.clone();
    let ui = Rc::downgrade(ui);

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

        if let Some(ui) = ui.upgrade() {
            ui.refresh(&app, true);
        }
    });
}

/// Reloads the theme whenever `~/.config/vaporeon/theme.toml` changes, so it's easy to tweak colors, fonts
/// and opacity while looking at the result.
fn watch_theme_file(app: &AppRef, ui: &Rc<Ui>) {
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
        let ui = Rc::downgrade(ui);
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
            if let Some(ui) = ui.upgrade() {
                ui.apply_theme(&theme);
            }
            app.borrow_mut().theme = theme;
            if let Some(ui) = ui.upgrade() {
                ui.refresh(&app, false);
            }
        }
    });

    app.borrow_mut().theme_file_monitor = Some(monitor);
}
