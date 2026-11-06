//! Runs git in the background: commits after saving, and keeps `storage_path` in sync with its git remote, if it has
//! one (see [`git::sync`]).
//!
//! Git runs in GIO's thread pool, so the UI never waits for it (or for the network). Only one git command runs at a
//! time: requests are queued (see [`Sync::run`]). Syncing happens:
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
    /// Whether git is running (see [`Sync::run`]).
    running: Cell<bool>,
    /// Waiting to run.
    commit_pending: Cell<bool>,
    sync_pending: Cell<bool>,
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
            commit_pending: Cell::new(false),
            sync_pending: Cell::new(false),
            last_sync: Cell::new(None),
            last_error: RefCell::new(None),
            push_timer: RefCell::new(None),
        })
    }

    /// Commits in the background (if `storage_path` is a git repository), and syncs a little later.
    pub fn commit(self: &Rc<Self>) {
        self.commit_pending.set(true);
        self.run();
    }

    /// Syncs in the background.
    pub fn sync_now(self: &Rc<Self>) {
        self.sync_pending.set(true);
        self.run();
    }

    /// Runs what's pending, one thing at a time, until there's nothing left. Asking for something while running just
    /// marks it as pending: the loop below picks it up.
    fn run(self: &Rc<Self>) {
        if self.running.replace(true) {
            return;
        }

        // A strong reference: once started, it runs until there's nothing pending.
        glib::spawn_future_local(glib::clone!(
            #[strong(rename_to = this)]
            self,
            async move {
                loop {
                    let path = this.path.clone();
                    let started_at = Instant::now();

                    // Syncing commits too, so there's no need to do both.
                    if this.sync_pending.take() {
                        this.commit_pending.set(false);
                        let result = gio::spawn_blocking(move || git::sync(&path))
                            .await
                            .unwrap_or_else(|_| Err("sync crashed".to_string()));
                        log::debug!("Synced in {} ms: {result:?}", started_at.elapsed().as_millis());
                        this.last_sync.set(Some(Instant::now()));
                        this.report(result);
                    } else if this.commit_pending.take() {
                        let result = gio::spawn_blocking(move || git::commit_all(&path)).await;
                        log::debug!("Committed in {} ms: {result:?}", started_at.elapsed().as_millis());
                        match result {
                            // Push it, a little later.
                            Ok(Ok(true)) => this.sync_soon(),
                            Ok(Ok(false)) => {}
                            // E.g. storage_path isn't a git repository, which is fine.
                            Ok(Err(err)) => log::debug!("Not committed: {}", err.message()),
                            Err(_) => log::error!("Committing crashed"),
                        }
                    } else {
                        break;
                    }
                }
                this.running.set(false);
            }
        ));
    }

    /// Syncs a little later. Used after committing, so a burst of changes results in a single push.
    pub fn sync_soon(self: &Rc<Self>) {
        if self.push_timer.borrow().is_some() {
            return;
        }
        let id = glib::timeout_add_local_once(
            PUSH_DELAY,
            glib::clone!(
                #[weak(rename_to = this)]
                self,
                move || {
                    this.push_timer.borrow_mut().take();
                    this.sync_now();
                }
            ),
        );
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
