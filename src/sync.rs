//! Keeps `storage_path` in sync with its git remote, if it has one. See [`git::sync`].
//!
//! Git runs in a background thread, so the UI never waits for the network. Syncing happens:
//! - when starting,
//! - a minute after committing (so a burst of changes is pushed once),
//! - every few minutes,
//! - when the window gets the focus (e.g. coming back to the computer after adding something from the phone),
//! - and when closing (see `main.rs`; the window is hidden by then).
//!
//! Changes pulled from the remote change the files on disk, which the file watchers notice like any other external
//! change: they're reloaded, or the user is asked what to do if there are unsaved changes.

use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::{Rc, Weak},
    time::{Duration, Instant},
};

use gtk::{gio, glib};

use crate::{AppRef, git, view::Ui};

/// How long to wait after committing before pushing.
const PUSH_DELAY: Duration = Duration::from_secs(60);
/// How often to sync anyway.
pub const PERIOD: Duration = Duration::from_secs(5 * 60);
/// When the window gets the focus, sync only if the last sync is older than this.
const FOCUS_MIN_AGE: Duration = Duration::from_secs(60);

pub struct Sync {
    path: PathBuf,
    app: Weak<RefCell<crate::App>>,
    ui: Weak<Ui>,
    running: Cell<bool>,
    /// Asked to sync while a sync was running: sync again when it's done.
    again: Cell<bool>,
    last_sync: Cell<Option<Instant>>,
    /// Last error reported, so the same one isn't reported again and again (e.g. while offline).
    last_error: RefCell<Option<String>>,
    push_timer: RefCell<Option<glib::SourceId>>,
}

impl Sync {
    pub fn new(path: PathBuf, app: &AppRef, ui: &Rc<Ui>) -> Rc<Self> {
        Rc::new(Self {
            path,
            app: Rc::downgrade(app),
            ui: Rc::downgrade(ui),
            running: Cell::new(false),
            again: Cell::new(false),
            last_sync: Cell::new(None),
            last_error: RefCell::new(None),
            push_timer: RefCell::new(None),
        })
    }

    /// Syncs in the background.
    pub fn sync_now(self: &Rc<Self>) {
        if self.running.replace(true) {
            self.again.set(true);
            return;
        }

        let this = self.clone();
        let path = self.path.clone();
        glib::spawn_future_local(async move {
            let started_at = Instant::now();
            let result = gio::spawn_blocking(move || git::sync(&path))
                .await
                .unwrap_or_else(|_| Err("sync crashed".to_string()));
            log::debug!("Synced in {} ms: {result:?}", started_at.elapsed().as_millis());

            this.running.set(false);
            this.last_sync.set(Some(Instant::now()));
            this.report(result);

            if this.again.replace(false) {
                this.sync_now();
            }
        });
    }

    /// Syncs a little later. Used after committing, so a burst of changes results in a single push.
    pub fn sync_soon(self: &Rc<Self>) {
        if self.push_timer.borrow().is_some() {
            return;
        }
        let this = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(PUSH_DELAY, move || {
            if let Some(this) = this.upgrade() {
                this.push_timer.borrow_mut().take();
                this.sync_now();
            }
        });
        *self.push_timer.borrow_mut() = Some(id);
    }

    /// Syncs, unless it synced recently.
    pub fn sync_if_stale(self: &Rc<Self>) {
        if self.last_sync.get().is_none_or(|last| last.elapsed() >= FOCUS_MIN_AGE) {
            self.sync_now();
        }
    }

    /// Shows errors in the status bar, but not the same one repeatedly.
    fn report(&self, result: Result<(), String>) {
        let (Some(app), Some(ui)) = (self.app.upgrade(), self.ui.upgrade()) else {
            return;
        };
        match result {
            Ok(()) => {
                if self.last_error.borrow_mut().take().is_some() {
                    app.borrow_mut()
                        .root
                        .info("Synced with the git remote again.".to_string());
                    ui.refresh(&app, false);
                }
            }
            Err(err) => {
                let is_new = self.last_error.borrow().as_deref() != Some(err.as_str());
                if is_new {
                    app.borrow_mut()
                        .root
                        .error(format!("Couldn't sync with the git remote: {err}"));
                    ui.refresh(&app, false);
                }
                *self.last_error.borrow_mut() = Some(err);
            }
        }
    }
}
