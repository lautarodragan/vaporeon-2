#![allow(clippy::new_without_default)]

use std::{
    cell::{Cell, RefCell},
    error::Error,
    rc::Rc,
    time::Instant,
};

use chrono::Local;
use flexi_logger::{FileSpec, WriteMode};
use gtk::{gdk, gio, glib, prelude::*};

use crate::{
    action::{Action, Actions, ApplicationAction, KeyCode, KeyEvent, KeyModifiers, OnActions, TextAction},
    components::root::{DataFile, EditTarget, Root, SaveResult},
    log_format::log_format,
    settings::Settings,
    state::{State, WindowState, next_zoom},
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
mod state;
mod sync;
mod theme;
mod toml_io;
mod view;

/// Debug builds (`cargo run`) get their own id, so they can run alongside the installed build (the app is
/// single-instance).
const APP_ID: &str = if cfg!(debug_assertions) {
    "codes.taro.Vaporeon.debug"
} else {
    "codes.taro.Vaporeon"
};

pub struct App {
    root: Root,
    settings: Settings,
    /// The theme as configured in theme.toml.
    base_theme: Theme,
    /// How big everything is: 1.0 = as configured in the theme. See [`set_zoom`].
    zoom: f64,
    /// The theme actually used: `base_theme`, zoomed.
    theme: Theme,
    actions: Actions,
    /// Kept around because the file monitor stops working when dropped.
    theme_file_monitor: Option<gio::FileMonitor>,
    /// Syncing with the git remote. Set once the UI exists.
    sync: Option<Rc<sync::Sync>>,
}

pub type AppRef = Rc<RefCell<App>>;

/// When the process started. Used to log how long starting up takes.
static STARTED_AT: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

fn ms_since(start: Instant) -> u128 {
    start.elapsed().as_millis()
}

fn ms_since_started() -> u128 {
    STARTED_AT.get().map(|s| ms_since(*s)).unwrap_or_default()
}

/// Logs that a phase of starting up is done, and when. To find out what's slow when starting feels slow.
fn startup_phase(phase: &str) {
    log::debug!("{phase}, {} ms after starting", ms_since_started());
}

fn main() -> Result<(), Box<dyn Error>> {
    STARTED_AT.get_or_init(Instant::now);

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
    startup_phase("GTK initialized");
    log::debug!(
        "GTK {}.{}.{}, display: {}",
        gtk::major_version(),
        gtk::minor_version(),
        gtk::micro_version(),
        gdk::Display::default()
            .map(|d| d.type_().name().to_string())
            .unwrap_or_default(),
    );

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
    startup_phase("Loaded settings and theme");

    let mut root = Root::new(settings.clone());
    root.load_selected_day();
    root.load_global();
    startup_phase("Loaded tasks");

    // Window size and zoom, as they were when the app was last closed.
    let state = State::load();
    let window_state = state.window;

    let app: AppRef = Rc::new(RefCell::new(App {
        root,
        settings,
        theme: theme.zoomed(state.zoom),
        base_theme: theme,
        zoom: state.zoom,
        actions,
        theme_file_monitor: None,
        sync: None,
    }));

    let window = gtk::ApplicationWindow::builder()
        .application(application)
        .title("Vaporeon")
        .default_width(window_state.width)
        .default_height(window_state.height)
        .maximized(window_state.maximized)
        .build();
    window.add_css_class("vaporeon");

    let ui = Ui::new(&app, window.clone());
    let storage_path = app.borrow().root.storage_path().to_path_buf();
    let sync = sync::Sync::new(storage_path.clone(), &app, &ui);
    app.borrow_mut().sync = Some(sync.clone());
    startup_phase("Built the UI");
    ui.apply_theme(&app.borrow().theme, app.borrow().zoom);
    startup_phase("Applied the theme (CSS, fonts)");

    setup_input(&app, &ui);
    watch_theme_file(&app, &ui);

    window.connect_close_request(glib::clone!(
        #[weak]
        app,
        #[weak]
        ui,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |window| {
            let closing_at = Instant::now();

            // Remember the window size. When maximized, GTK keeps the size from before maximizing, which is what we
            // want to restore when unmaximizing next time.
            let (width, height) = window.default_size();
            State {
                window: WindowState {
                    width,
                    height,
                    maximized: window.is_maximized(),
                },
                zoom: app.borrow().zoom,
            }
            .save();

            // Hide the window right away, and only then save: saving (and committing) can take a while,
            // e.g. if git signs commits, and there's no reason to keep the window on screen meanwhile.
            window.set_visible(false);
            WidgetExt::display(window).flush();

            let conflicts = ui.pending_conflicts();
            let mut app = app.borrow_mut();
            // Whatever is being edited is kept, as if confirmed.
            app.root.finish_editing();
            for file in [DataFile::Global, DataFile::Day] {
                if conflicts.contains(&file) {
                    // It changed on disk and the user didn't choose what to do yet: don't overwrite it.
                    // Their version goes to a backup instead.
                    if let Err(err) = app.root.backup(file) {
                        log::error!("Couldn't save a backup of {file:?}: {err:?}");
                    }
                } else {
                    app.root.save_file(file);
                }
            }
            app.settings.save();
            // The UI is going away: nobody's going to take care of these.
            app.root.take_events();

            // Commit, and push what's not pushed yet (pulling first, so the push isn't rejected). Right here, not in
            // the background: the app is about to quit. The window is already hidden.
            // If git is busy in the background (e.g. a periodic sync), this may fail. Nothing's lost: whatever isn't
            // committed or pushed will be next time.
            if let Err(err) = git::commit_all(&storage_path) {
                log::debug!("Not committed: {}", err.message());
            }
            if let Err(err) = git::sync(&storage_path) {
                log::warn!("Couldn't sync with the git remote: {err}");
            }
            log::debug!("Saved everything in {} ms. Closing.", ms_since(closing_at));
            glib::Propagation::Proceed
        }
    ));

    // The week view highlights "today", so redraw it when the day changes.
    let mut today = Local::now().date_naive();
    glib::timeout_add_seconds_local(
        30,
        glib::clone!(
            #[weak]
            ui,
            #[upgrade_or]
            glib::ControlFlow::Break,
            move || {
                let now = Local::now().date_naive();
                if now != today {
                    today = now;
                    ui.redraw_header();
                }
                glib::ControlFlow::Continue
            }
        ),
    );

    window.connect_realize(|_| startup_phase("Window realized"));
    window.connect_map(|_| startup_phase("Window mapped"));
    window.present();
    startup_phase("Window presented");

    // Log how long it took to show something on screen. Ticks happen right before drawing a frame.
    window.add_tick_callback(|window, _| {
        startup_phase("First frame");
        let renderer = window.renderer().map(|r| r.type_().name().to_string());
        log::debug!("Renderer: {}", renderer.unwrap_or_default());
        glib::ControlFlow::Break
    });
    // After presenting, so the keyboard focus can be set.
    ui.refresh(&app, true);
    startup_phase("Refreshed the UI");

    sync.sync_now();
    glib::timeout_add_local(
        sync::PERIOD,
        glib::clone!(
            #[weak]
            sync,
            #[upgrade_or]
            glib::ControlFlow::Break,
            move || {
                sync.sync_now();
                glib::ControlFlow::Continue
            }
        ),
    );
    // E.g. coming back to the computer after adding tasks from the phone.
    window.connect_is_active_notify(glib::clone!(
        #[weak]
        sync,
        move |window| {
            if window.is_active() {
                sync.sync_if_stale();
            }
        }
    ));
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

/// Zooms in (`direction` > 0), out (< 0), or back to 100% (0).
fn zoom(app: &AppRef, ui: &Rc<Ui>, direction: i32) {
    let zoom = match direction {
        0 => 1.0,
        _ => next_zoom(app.borrow().zoom, direction),
    };
    set_zoom(app, ui, zoom);
}

fn set_zoom(app: &AppRef, ui: &Rc<Ui>, zoom: f64) {
    {
        let mut app = app.borrow_mut();
        if app.zoom == zoom {
            return;
        }
        app.zoom = zoom;
        app.theme = app.base_theme.zoomed(zoom);
        app.root.info(format!("Zoom: {}%", (zoom * 100.0).round()));
    }
    ui.apply_theme(&app.borrow().theme, zoom);
    ui.refresh(app, true);
}

fn setup_input(app: &AppRef, ui: &Rc<Ui>) {
    // Ctrl + mouse wheel (or touchpad scrolling) zooms, like in browsers.
    // Capture phase, so it happens before the lists scroll.
    let scroll_controller = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    scroll_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    let scrolled = Rc::new(Cell::new(0.0));
    scroll_controller.connect_scroll(glib::clone!(
        #[weak]
        app,
        #[weak]
        ui,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |controller, _, dy| {
            if !controller
                .current_event_state()
                .contains(gdk::ModifierType::CONTROL_MASK)
            {
                return glib::Propagation::Proceed;
            }
            if ui.is_prompt_active() {
                return glib::Propagation::Stop;
            }
            // A mouse wheel scrolls one unit per notch; touchpads scroll in many small steps. Zoom once per unit.
            scrolled.set(scrolled.get() + dy);
            while scrolled.get().abs() >= 1.0 {
                let direction = -scrolled.get().signum();
                scrolled.set(scrolled.get() + direction);
                zoom(&app, &ui, direction as i32);
            }
            glib::Propagation::Stop
        }
    ));
    ui.window.add_controller(scroll_controller);

    let key_controller = gtk::EventControllerKey::new();

    // Capture phase: we see key presses before the focused widget (e.g. a text editor) does.
    key_controller.set_propagation_phase(gtk::PropagationPhase::Capture);

    // This handler holds the only strong reference to `ui` (everything else holds weak ones),
    // so the UI lives as long as the window does.
    key_controller.connect_key_pressed(glib::clone!(
        #[strong]
        app,
        #[strong]
        ui,
        move |_, key, _keycode, state| {
            let Some(event) = KeyEvent::from_gdk(key, state) else {
                return glib::Propagation::Proceed;
            };

            if std::env::args().any(|arg| arg == "--log-events") {
                log::debug!("event: {event:?} ({key:?}, {state:?})");
            }

            // While a question is shown in the status bar, only its options respond.
            if ui.is_prompt_active() {
                if event.code == KeyCode::Esc {
                    ui.dismiss_prompt();
                } else if let KeyCode::Char(key) = event.code
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

            for (action, direction) in [
                (ApplicationAction::ZoomIn, 1),
                (ApplicationAction::ZoomOut, -1),
                (ApplicationAction::ZoomReset, 0),
            ] {
                if actions.contains(&Action::Application(action)) {
                    zoom(&app, &ui, direction);
                    return glib::Propagation::Stop;
                }
            }

            if actions.contains(&Action::Application(ApplicationAction::Save)) {
                {
                    let mut app = app.borrow_mut();
                    match app.root.save_all() {
                        SaveResult::Saved => {
                            let date = app.root.selected_date();
                            app.root.info(format!("Saved {date}."));
                        }
                        SaveResult::Unchanged => app.root.info("No changes to save.".to_string()),
                        SaveResult::Failed => {} // The error was already reported.
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

            let undo = actions.contains(&Action::Application(ApplicationAction::Undo));
            if undo || actions.contains(&Action::Application(ApplicationAction::Redo)) {
                {
                    let mut app = app.borrow_mut();
                    let done = if undo { app.root.undo() } else { app.root.redo() };
                    if !done {
                        let what = if undo { "undo" } else { "redo" };
                        app.root.info(format!("Nothing to {what}."));
                    }
                }
                ui.refresh(&app, true);
                return glib::Propagation::Stop;
            }

            if actions.contains(&Action::Application(ApplicationAction::Paste)) {
                paste(&app, &ui);
            } else {
                app.borrow_mut().root.on_actions(actions);
                ui.refresh(&app, true);
            }

            glib::Propagation::Stop
        }
    ));

    ui.window.add_controller(key_controller);
}

/// Reads text from the clipboard and adds it as a new task.
/// (While editing, pasting is handled by the text editor itself.)
fn paste(app: &AppRef, ui: &Rc<Ui>) {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let clipboard = display.clipboard();

    glib::spawn_future_local(glib::clone!(
        #[weak]
        app,
        #[weak]
        ui,
        async move {
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
            ui.refresh(&app, true);
        }
    ));
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

    monitor.connect_changed(glib::clone!(
        #[weak]
        app,
        #[weak]
        ui,
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
            {
                let mut app = app.borrow_mut();
                app.theme = theme.zoomed(app.zoom);
                app.base_theme = theme;
            }
            ui.apply_theme(&app.borrow().theme, app.borrow().zoom);
            ui.refresh(&app, false);
        }
    ));

    app.borrow_mut().theme_file_monitor = Some(monitor);
}
