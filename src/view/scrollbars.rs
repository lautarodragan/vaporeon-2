//! The lists and the details scroll without scrollbars: the edges fade out instead, when there's more to scroll to
//! (see [`super::fade_edges`]). Unless the user's settings ask to always show scrollbars
//! (`gtk-overlay-scrolling = false`, e.g. GNOME's "Always Show Scroll Bars").

use gtk::glib;

/// Hides the scrollbars of `scrolled` that would otherwise be shown automatically, following the setting if it
/// changes.
pub fn hide(scrolled: &gtk::ScrolledWindow) {
    let Some(settings) = gtk::Settings::default() else {
        return;
    };
    let automatic = (
        scrolled.hscrollbar_policy() == gtk::PolicyType::Automatic,
        scrolled.vscrollbar_policy() == gtk::PolicyType::Automatic,
    );
    let update = move |scrolled: &gtk::ScrolledWindow, settings: &gtk::Settings| {
        // `External` scrolls like `Automatic`, without showing the scrollbar.
        let policy = if settings.is_gtk_overlay_scrolling() {
            gtk::PolicyType::External
        } else {
            gtk::PolicyType::Automatic
        };
        let (h, v) = scrolled.policy();
        scrolled.set_policy(
            if automatic.0 { policy } else { h },
            if automatic.1 { policy } else { v },
        );
    };
    update(scrolled, &settings);
    settings.connect_gtk_overlay_scrolling_notify(glib::clone!(
        #[weak]
        scrolled,
        move |settings| update(&scrolled, settings)
    ));
}
