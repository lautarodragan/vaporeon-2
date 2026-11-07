//! A container for a [`gtk::ScrolledWindow`] that fades out the edges with more content past them, so it's clear
//! there's more to scroll to (the scrollbars are hidden, see [`super::scrollbars`]).
//!
//! It's an alpha mask: the content itself becomes transparent towards the edge, so it works over the transparent
//! window. Painting the background color over it (e.g. with the CSS `undershoot` nodes) would make the edges more
//! opaque than the rest of the window.

use std::cell::Cell;

use gtk::{glib, graphene, gsk, prelude::*, subclass::prelude::*};

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct FadeEdges {
        /// How far the fade reaches into the content, in pixels.
        pub size: Cell<f32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FadeEdges {
        const NAME: &'static str = "VaporeonFadeEdges";
        type Type = super::FadeEdges;
        type ParentType = gtk::Widget;

        fn class_init(class: &mut Self::Class) {
            class.set_layout_manager_type::<gtk::BinLayout>();
        }
    }

    impl ObjectImpl for FadeEdges {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for FadeEdges {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let Some(scrolled) = widget.first_child().and_downcast::<gtk::ScrolledWindow>() else {
                self.parent_snapshot(snapshot);
                return;
            };
            let (width, height) = (widget.width() as f32, widget.height() as f32);
            let size = self.size.get();
            let bounds = graphene::Rect::new(0.0, 0.0, width, height);

            let (top, bottom) = more_content(&scrolled.vadjustment());
            let (left, right) = more_content(&scrolled.hadjustment());
            let vertical = (top || bottom) && height > 2.0 * size;
            let horizontal = (left || right) && width > 2.0 * size;

            // A mask for each direction: opaque, except for a gradient to transparent at each edge that fades.
            let push_mask = |start: bool, end: bool, length: f32, to: graphene::Point| {
                let fade = size / length;
                let stops = [
                    gsk::ColorStop::new(0.0, mask_color(!start)),
                    gsk::ColorStop::new(fade, mask_color(true)),
                    gsk::ColorStop::new(1.0 - fade, mask_color(true)),
                    gsk::ColorStop::new(1.0, mask_color(!end)),
                ];
                snapshot.push_mask(gsk::MaskMode::Alpha);
                snapshot.append_linear_gradient(&bounds, &graphene::Point::zero(), &to, &stops);
                snapshot.pop();
            };
            if vertical {
                push_mask(top, bottom, height, graphene::Point::new(0.0, height));
            }
            if horizontal {
                push_mask(left, right, width, graphene::Point::new(width, 0.0));
            }

            self.parent_snapshot(snapshot);

            for _ in 0..(vertical as u8 + horizontal as u8) {
                snapshot.pop();
            }
        }
    }
}

glib::wrapper! {
    pub struct FadeEdges(ObjectSubclass<imp::FadeEdges>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl FadeEdges {
    pub fn new(scrolled: &gtk::ScrolledWindow) -> Self {
        let widget: Self = glib::Object::new();
        widget.imp().size.set(24.0);
        scrolled.set_parent(&widget);

        // Which edges fade depends on the scroll position and the size of the content.
        for adjustment in [scrolled.vadjustment(), scrolled.hadjustment()] {
            adjustment.connect_value_changed(glib::clone!(
                #[weak]
                widget,
                move |_| widget.queue_draw()
            ));
            adjustment.connect_changed(glib::clone!(
                #[weak]
                widget,
                move |_| widget.queue_draw()
            ));
        }
        widget
    }

    pub fn set_size(&self, size: f64) {
        if self.imp().size.replace(size as f32) != size as f32 {
            self.queue_draw();
        }
    }
}

/// Whether there's more content before and after what's shown.
fn more_content(adjustment: &gtk::Adjustment) -> (bool, bool) {
    let value = adjustment.value();
    (value > 0.5, value + adjustment.page_size() < adjustment.upper() - 0.5)
}

fn mask_color(opaque: bool) -> gtk::gdk::RGBA {
    gtk::gdk::RGBA::new(0.0, 0.0, 0.0, if opaque { 1.0 } else { 0.0 })
}
