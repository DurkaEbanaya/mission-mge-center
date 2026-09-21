/* preferences/appearance_page.rs
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

use std::cell::{Cell, RefCell};

use adw::{prelude::*, subclass::prelude::*, ButtonRow, ComboRow};
use glib::g_critical;
use gtk::{gio, glib, Scale, StringList};

use crate::acrylic_style::SETTING_KEYS;
use crate::settings;

const PRESETS: [[i32; 5]; 4] = [
    [20, 3, 5, 2, 8],
    [46, 8, 9, 6, 13],
    [68, 14, 13, 9, 18],
    [100, 0, 0, 0, 20],
];

mod imp {
    use super::*;

    #[derive(gtk::CompositeTemplate, Default)]
    #[template(resource = "/io/missioncenter/MissionCenter/ui/preferences/appearance_page.ui")]
    pub struct PreferencesAppearancePage {
        #[template_child]
        pub preset: TemplateChild<ComboRow>,
        #[template_child]
        pub tint_opacity: TemplateChild<Scale>,
        #[template_child]
        pub noise_opacity: TemplateChild<Scale>,
        #[template_child]
        pub highlight_opacity: TemplateChild<Scale>,
        #[template_child]
        pub shadow_opacity: TemplateChild<Scale>,
        #[template_child]
        pub divider_opacity: TemplateChild<Scale>,
        #[template_child]
        pub reset: TemplateChild<ButtonRow>,
        pub updating: Cell<bool>,
        pub settings_handlers: RefCell<Vec<glib::SignalHandlerId>>,
    }

    impl PreferencesAppearancePage {
        fn scales(&self) -> [&Scale; 5] {
            [
                &self.tint_opacity,
                &self.noise_opacity,
                &self.highlight_opacity,
                &self.shadow_opacity,
                &self.divider_opacity,
            ]
        }

        fn values(&self, settings: &gio::Settings) -> [i32; 5] {
            SETTING_KEYS.map(|key| settings.int(key))
        }

        pub(super) fn sync(&self, settings: &gio::Settings) {
            self.updating.set(true);
            let values = self.values(settings);

            for (scale, value) in self.scales().into_iter().zip(values) {
                scale.set_value(value as f64);
            }
            self.preset.set_selected(
                PRESETS
                    .iter()
                    .position(|preset| *preset == values)
                    .unwrap_or(4) as u32,
            );
            self.updating.set(false);
        }

        fn apply_values(&self, values: [i32; 5]) {
            let settings = settings!();

            for (key, value) in SETTING_KEYS.into_iter().zip(values) {
                if let Err(error) = settings.set_int(key, value) {
                    g_critical!("MissionCenter::Preferences", "Failed to set {key}: {error}");
                }
            }
            self.sync(&settings);
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PreferencesAppearancePage {
        const NAME: &'static str = "PreferencesAppearancePage";
        type Type = super::PreferencesAppearancePage;
        type ParentType = adw::PreferencesPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PreferencesAppearancePage {
        fn constructed(&self) {
            self.parent_constructed();
            self.updating.set(true);
            self.preset.set_model(Some(&StringList::new(&[
                "Clear",
                "Windows 10 Acrylic",
                "Frosted",
                "Opaque",
                "Custom",
            ])));
            self.updating.set(false);

            self.preset.connect_selected_notify({
                let this = self.obj().downgrade();
                move |_| {
                    let Some(this) = this.upgrade() else {
                        return;
                    };
                    let imp = this.imp();
                    if imp.updating.get() {
                        return;
                    }
                    if let Some(values) = PRESETS.get(imp.preset.selected() as usize) {
                        imp.apply_values(*values);
                    } else {
                        imp.sync(&settings!());
                    }
                }
            });

            for (scale, key) in self.scales().into_iter().zip(SETTING_KEYS) {
                scale.set_format_value_func(|_, value| format!("{value:.0}%"));
                scale.connect_value_changed({
                    let this = self.obj().downgrade();
                    move |scale| {
                        let Some(this) = this.upgrade() else {
                            return;
                        };
                        let imp = this.imp();
                        if imp.updating.get() {
                            return;
                        }

                        let settings = settings!();
                        if let Err(error) = settings.set_int(key, scale.value().round() as i32) {
                            g_critical!(
                                "MissionCenter::Preferences",
                                "Failed to set {key}: {error}"
                            );
                        }
                        imp.sync(&settings);
                    }
                });
            }

            for key in SETTING_KEYS {
                let settings = settings!();
                let handler = settings.connect_changed(Some(key), {
                    let this = self.obj().downgrade();
                    move |settings, _| {
                        if let Some(this) = this.upgrade() {
                            this.imp().sync(settings);
                        }
                    }
                });
                self.settings_handlers.borrow_mut().push(handler);
            }

            self.reset.connect_activated({
                let this = self.obj().downgrade();
                move |_| {
                    if let Some(this) = this.upgrade() {
                        this.imp().apply_values(PRESETS[1]);
                    }
                }
            });
        }

        fn dispose(&self) {
            let settings = settings!();
            for handler in self.settings_handlers.take() {
                settings.disconnect(handler);
            }
        }
    }

    impl WidgetImpl for PreferencesAppearancePage {}
    impl PreferencesPageImpl for PreferencesAppearancePage {}
}

glib::wrapper! {
    pub struct PreferencesAppearancePage(ObjectSubclass<imp::PreferencesAppearancePage>)
        @extends adw::PreferencesPage, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::ConstraintTarget, gtk::Accessible, gtk::Buildable;
}

impl PreferencesAppearancePage {
    pub fn new() -> Self {
        let this: Self = glib::Object::builder().build();
        this.imp().sync(&settings!());
        this
    }
}
