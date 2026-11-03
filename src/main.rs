#![allow(clippy::new_without_default)]

use std::{cell::RefCell, error::Error, rc::Rc};

use flexi_logger::{FileSpec, WriteMode};
use gtk::{gdk, gio, glib, prelude::*};

use crate::{
    action::{Action, Actions, ApplicationAction, KeyEvent, KeyModifiers, OnActions, TextAction},
    canvas::Canvas,
    components::root::Root,
    log_format::log_format,
    settings::Settings,
    theme::Theme,
    toml_io::get_config_file_path,
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

struct App {
    root: Root,
    settings: Settings,
    theme: Theme,
    actions: Actions,
    /// Kept around because the file monitor stops working when dropped.
    theme_file_monitor: Option<gio::FileMonitor>,
}

type AppRef = Rc<RefCell<App>>;

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

    drawing_area.set_draw_func({
        let app = app.clone();
        move |area, cr, width, height| {
            let mut app = app.borrow_mut();
            let App { root, theme, .. } = &mut *app;
            let canvas = Canvas::new(cr, &area.pango_context(), theme, width as f64, height as f64);
            root.set_visible_rows(Root::visible_rows(&canvas));
            root.refresh_scroll();
            root.render(&canvas);
        }
    });

    setup_input(&app, &window, &drawing_area);
    watch_theme_file(&app, &window, &drawing_area);

    window.connect_close_request({
        let app = app.clone();
        move |_| {
            let app = app.borrow();
            app.root.save_global();
            app.settings.save();
            glib::Propagation::Proceed
        }
    });

    // The week view highlights "today", so redraw every now and then in case the day changes.
    glib::timeout_add_seconds_local(30, {
        let drawing_area = drawing_area.downgrade();
        move || match drawing_area.upgrade() {
            Some(drawing_area) => {
                drawing_area.queue_draw();
                glib::ControlFlow::Continue
            }
            None => glib::ControlFlow::Break,
        }
    });

    window.set_child(Some(&drawing_area));
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

fn setup_input(app: &AppRef, window: &gtk::ApplicationWindow, drawing_area: &gtk::DrawingArea) {
    // The input method context takes care of dead keys (´ + a = á), compose sequences, and IMEs.
    // We only route key presses through it while editing text, so single-key shortcuts keep working otherwise.
    let im_context = gtk::IMMulticontext::new();
    im_context.set_client_widget(Some(drawing_area));
    im_context.set_use_preedit(false);

    im_context.connect_commit({
        let app = app.clone();
        let drawing_area = drawing_area.downgrade();
        move |_, text| {
            let actions = text
                .chars()
                .map(|c| Action::Text(TextAction::Char(c)))
                .collect::<Vec<_>>();
            for action in actions {
                app.borrow_mut().root.on_actions(vec![action]);
            }
            if let Some(drawing_area) = drawing_area.upgrade() {
                drawing_area.queue_draw();
            }
        }
    });

    let focus_controller = gtk::EventControllerFocus::new();
    focus_controller.connect_enter({
        let im_context = im_context.clone();
        move |_| im_context.focus_in()
    });
    focus_controller.connect_leave({
        let im_context = im_context.clone();
        move |_| im_context.focus_out()
    });
    drawing_area.add_controller(focus_controller);

    let key_controller = gtk::EventControllerKey::new();

    key_controller.connect_key_pressed({
        let app = app.clone();
        let window = window.downgrade();
        let drawing_area = drawing_area.downgrade();
        let im_context = im_context.clone();
        move |controller, key, _keycode, state| {
            let log_events = std::env::args().any(|arg| arg == "--log-events");
            let is_editing = app.borrow().root.is_editing();
            let modifiers = KeyModifiers::from_gdk(state);
            let is_plain_key = !modifiers.contains(KeyModifiers::CONTROL) && !modifiers.contains(KeyModifiers::ALT);

            // This must happen before converting to a KeyEvent: dead keys (like ´) don't map to any KeyEvent.
            // Note: `filter_keypress` may synchronously emit `commit`, which borrows `app`,
            // so we must not be holding a borrow here.
            if is_editing
                && is_plain_key
                && let Some(gdk_event) = controller.current_event()
                && im_context.filter_keypress(&gdk_event)
            {
                if log_events {
                    log::debug!("event: {key:?} {state:?} (handled by input method)");
                }
                return glib::Propagation::Stop;
            }

            let Some(event) = KeyEvent::from_gdk(key, state) else {
                return glib::Propagation::Proceed;
            };

            if log_events {
                log::debug!("event: {event:?} ({key:?}, {state:?})");
            }

            let actions = app.borrow().actions.actions_by_key(event);

            if actions.contains(&Action::Application(ApplicationAction::Quit)) {
                if let Some(window) = window.upgrade() {
                    window.close();
                }
                return glib::Propagation::Stop;
            } else if actions.contains(&Action::Application(ApplicationAction::Save)) {
                let app = app.borrow();
                app.root.save();
                app.root.save_global();
            } else if actions.contains(&Action::Application(ApplicationAction::Paste)) {
                paste(&app, &drawing_area);
            } else {
                app.borrow_mut().root.on_actions(actions);
            }

            if let Some(drawing_area) = drawing_area.upgrade() {
                drawing_area.queue_draw();
            }

            glib::Propagation::Stop
        }
    });

    key_controller.connect_key_released({
        let app = app.clone();
        let im_context = im_context.clone();
        move |controller, _key, _keycode, _state| {
            let is_editing = app.borrow().root.is_editing();
            if is_editing && let Some(gdk_event) = controller.current_event() {
                im_context.filter_keypress(&gdk_event);
            }
        }
    });

    // Attach to the window rather than the drawing area, so keys work regardless of what has focus.
    window.add_controller(key_controller);
}

/// Reads text from the clipboard and pastes it.
///
/// While editing text, the pasted text is inserted at the caret. Otherwise, it's added as a new task.
fn paste(app: &AppRef, drawing_area: &glib::WeakRef<gtk::DrawingArea>) {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let clipboard = display.clipboard();
    let app = app.clone();
    let drawing_area = drawing_area.clone();

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

        let mut app = app.borrow_mut();
        if app.root.is_editing() {
            for c in text.chars() {
                let action = if c == '\n' {
                    Action::Text(TextAction::NewLine)
                } else {
                    Action::Text(TextAction::Char(c))
                };
                app.root.on_actions(vec![action]);
            }
        } else {
            app.root
                .on_actions(vec![Action::Application(ApplicationAction::PasteText(text))]);
        }
        drop(app);

        if let Some(drawing_area) = drawing_area.upgrade() {
            drawing_area.queue_draw();
        }
    });
}

/// Reloads the theme whenever `~/.config/vaporeon/theme.toml` changes, so it's easy to tweak colors, fonts
/// and opacity while looking at the result.
fn watch_theme_file(app: &AppRef, window: &gtk::ApplicationWindow, drawing_area: &gtk::DrawingArea) {
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
        let drawing_area = drawing_area.downgrade();
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
            app.borrow_mut().theme = theme;
            if let Some(drawing_area) = drawing_area.upgrade() {
                drawing_area.queue_draw();
            }
        }
    });

    app.borrow_mut().theme_file_monitor = Some(monitor);
}
