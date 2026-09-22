/* preferences/appearance_page.rs
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

use std::cell::{Cell, RefCell};
use std::ffi::OsStr;

use adw::{prelude::*, subclass::prelude::*, ActionRow, ButtonRow, ComboRow};
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
const KWIN_BLUR_MIN: i32 = 1;
const KWIN_BLUR_MAX: i32 = 15;
const KWIN_BLUR_DEFAULT: i32 = 15;
const KWIN_BLUR_UNSET: &str = "mission-center-unset";

#[derive(Clone, Copy, PartialEq, Eq)]
struct KwinBlurState {
    value: i32,
    explicitly_set: bool,
}

impl Default for KwinBlurState {
    fn default() -> Self {
        Self {
            value: KWIN_BLUR_DEFAULT,
            explicitly_set: false,
        }
    }
}

impl KwinBlurState {
    fn displayed_value(self) -> i32 {
        self.value.clamp(KWIN_BLUR_MIN, KWIN_BLUR_MAX)
    }
}

fn kwin_blur_tools_available() -> bool {
    std::env::var_os("FLATPAK_ID").is_none()
        && std::env::var_os("SNAP").is_none()
        && ["kreadconfig6", "kwriteconfig6", "qdbus6"]
            .into_iter()
            .all(|program| glib::find_program_in_path(program).is_some())
}

async fn run_command(arguments: Vec<String>) -> Result<String, String> {
    let launcher = gio::SubprocessLauncher::new(
        gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_PIPE,
    );
    launcher.unsetenv("LD_PRELOAD");
    let arguments = arguments.iter().map(OsStr::new).collect::<Vec<_>>();
    let process = launcher
        .spawn(&arguments)
        .map_err(|error| error.to_string())?;
    let (stdout, stderr) = process
        .communicate_utf8_future(None)
        .await
        .map_err(|error| error.to_string())?;
    if process.has_signaled() {
        return Err(format!(
            "{} terminated by signal {}",
            arguments[0].to_string_lossy(),
            process.term_sig()
        ));
    }
    if !process.has_exited() {
        return Err(format!(
            "{} did not exit normally",
            arguments[0].to_string_lossy()
        ));
    }
    if !process.is_successful() {
        let stderr = stderr.as_deref().unwrap_or_default().trim();
        return Err(if stderr.is_empty() {
            format!(
                "{} exited with status {}",
                arguments[0].to_string_lossy(),
                process.exit_status()
            )
        } else {
            stderr.to_owned()
        });
    }
    Ok(stdout.unwrap_or_default().into())
}

async fn kwin_blur_available() -> bool {
    if !kwin_blur_tools_available() {
        return false;
    }

    run_command(
        [
            "qdbus6",
            "org.kde.KWin",
            "/Effects",
            "org.kde.kwin.Effects.isEffectLoaded",
            "blur",
        ]
        .map(str::to_owned)
        .to_vec(),
    )
    .await
    .is_ok_and(|output| output.trim() == "true")
}

async fn kwin_blur_state() -> Result<KwinBlurState, String> {
    let output = run_command(
        [
            "kreadconfig6",
            "--file",
            "kwinrc",
            "--group",
            "Effect-blur",
            "--key",
            "BlurStrength",
            "--default",
            KWIN_BLUR_UNSET,
        ]
        .map(str::to_owned)
        .to_vec(),
    )
    .await?;
    let output = output.trim();
    if output == KWIN_BLUR_UNSET {
        return Ok(KwinBlurState {
            value: KWIN_BLUR_DEFAULT,
            explicitly_set: false,
        });
    }

    let value = output
        .parse::<i32>()
        .map_err(|error| format!("Invalid KWin blur strength {output:?}: {error}"))?;
    Ok(KwinBlurState {
        value,
        explicitly_set: true,
    })
}

async fn write_kwin_blur_state(state: KwinBlurState) -> Result<(), String> {
    let mut arguments = [
        "kwriteconfig6",
        "--file",
        "kwinrc",
        "--group",
        "Effect-blur",
        "--key",
        "BlurStrength",
    ]
    .map(str::to_owned)
    .to_vec();
    if state.explicitly_set {
        arguments.push(state.value.to_string());
    } else {
        arguments.extend(["--delete".to_owned(), String::new()]);
    }
    run_command(arguments).await.map(|_| ())
}

async fn reload_kwin_blur() -> Result<(), String> {
    run_command(
        [
            "qdbus6",
            "org.kde.KWin",
            "/Effects",
            "org.kde.kwin.Effects.reconfigureEffect",
            "blur",
        ]
        .map(str::to_owned)
        .to_vec(),
    )
    .await
    .map(|_| ())
}

async fn set_kwin_blur_strength(
    value: i32,
    fallback: KwinBlurState,
) -> Result<KwinBlurState, (Option<KwinBlurState>, String)> {
    let previous = kwin_blur_state()
        .await
        .map_err(|error| (Some(fallback), error))?;
    let state = KwinBlurState {
        value: value.clamp(KWIN_BLUR_MIN, KWIN_BLUR_MAX),
        explicitly_set: true,
    };

    if let Err(error) = write_kwin_blur_state(state).await {
        let restored = kwin_blur_state()
            .await
            .ok()
            .filter(|current| *current == previous);
        return Err((restored, error));
    }
    if let Err(error) = reload_kwin_blur().await {
        let rollback = match write_kwin_blur_state(previous).await {
            Ok(()) => reload_kwin_blur().await,
            Err(error) => Err(error),
        };
        return match rollback {
            Ok(()) => Err((Some(previous), error)),
            Err(rollback) => Err((None, format!("{error}; rollback failed: {rollback}"))),
        };
    }
    Ok(state)
}

mod imp {
    use super::*;

    #[derive(gtk::CompositeTemplate, Default)]
    #[template(resource = "/io/missioncenter/MissionCenter/ui/preferences/appearance_page.ui")]
    pub struct PreferencesAppearancePage {
        #[template_child]
        pub preset: TemplateChild<ComboRow>,
        #[template_child]
        pub kwin_blur_row: TemplateChild<ActionRow>,
        #[template_child]
        pub blur_intensity: TemplateChild<Scale>,
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
        kwin_blur_state: Cell<KwinBlurState>,
        pub pending_kwin_blur: Cell<Option<i32>>,
        pub applying_kwin_blur: Cell<bool>,
        pub refreshing_kwin_blur: Cell<bool>,
        pub kwin_blur_refresh_requested: Cell<bool>,
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

        fn apply_pending_kwin_blur(&self) {
            if self.applying_kwin_blur.replace(true) {
                return;
            }
            let Some(value) = self.pending_kwin_blur.take() else {
                self.applying_kwin_blur.set(false);
                return;
            };
            let fallback = self.kwin_blur_state.get();
            let this = self.obj().downgrade();
            glib::MainContext::default().spawn_local(async move {
                let result = set_kwin_blur_strength(value, fallback).await;
                let Some(this) = this.upgrade() else {
                    return;
                };
                let imp = this.imp();
                imp.applying_kwin_blur.set(false);
                match result {
                    Ok(state) => imp.kwin_blur_state.set(state),
                    Err((state, error)) => {
                        g_critical!(
                            "MissionCenter::Preferences",
                            "Failed to set KWin blur strength: {error}"
                        );
                        imp.pending_kwin_blur.set(None);
                        if let Some(state) = state {
                            imp.kwin_blur_state.set(state);
                            imp.updating.set(true);
                            imp.blur_intensity.set_value(state.displayed_value() as f64);
                            imp.updating.set(false);
                        } else {
                            imp.kwin_blur_row.set_visible(false);
                        }
                    }
                }
                if imp.pending_kwin_blur.get().is_some() {
                    imp.apply_pending_kwin_blur();
                } else {
                    imp.maybe_refresh_kwin_blur();
                }
            });
        }

        pub(super) fn maybe_refresh_kwin_blur(&self) {
            if !self.kwin_blur_refresh_requested.get()
                || self.applying_kwin_blur.get()
                || self.pending_kwin_blur.get().is_some()
                || self.refreshing_kwin_blur.get()
            {
                return;
            }
            self.kwin_blur_refresh_requested.set(false);
            self.refreshing_kwin_blur.set(true);
            self.kwin_blur_row.set_sensitive(false);
            let this = self.obj().downgrade();
            glib::MainContext::default().spawn_local(async move {
                let available = kwin_blur_available().await;
                let Some(this) = this.upgrade() else {
                    return;
                };
                let imp = this.imp();
                if !available {
                    imp.kwin_blur_row.set_visible(false);
                    imp.finish_kwin_blur_refresh();
                    return;
                }
                let state = match kwin_blur_state().await {
                    Ok(state) => state,
                    Err(error) => {
                        g_critical!(
                            "MissionCenter::Preferences",
                            "Failed to read KWin blur strength: {error}"
                        );
                        imp.kwin_blur_row.set_visible(false);
                        imp.finish_kwin_blur_refresh();
                        return;
                    }
                };
                imp.kwin_blur_state.set(state);
                imp.updating.set(true);
                imp.blur_intensity.set_value(state.displayed_value() as f64);
                imp.updating.set(false);
                imp.kwin_blur_row.set_visible(true);
                imp.finish_kwin_blur_refresh();
            });
        }

        fn finish_kwin_blur_refresh(&self) {
            self.refreshing_kwin_blur.set(false);
            if self.kwin_blur_refresh_requested.get() {
                self.maybe_refresh_kwin_blur();
            } else {
                self.kwin_blur_row.set_sensitive(true);
            }
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

            self.blur_intensity
                .set_format_value_func(|_, value| format!("{value:.0} / {KWIN_BLUR_MAX}"));
            self.blur_intensity.connect_value_changed({
                let this = self.obj().downgrade();
                move |scale| {
                    let Some(this) = this.upgrade() else {
                        return;
                    };
                    let imp = this.imp();
                    if imp.updating.get() {
                        return;
                    }
                    let value = scale.value().round() as i32;
                    imp.pending_kwin_blur.set(Some(value));
                    imp.apply_pending_kwin_blur();
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
        let imp = this.imp();
        imp.sync(&settings!());
        this
    }

    pub fn refresh_kwin_blur(&self) {
        let imp = self.imp();
        imp.kwin_blur_refresh_requested.set(true);
        imp.maybe_refresh_kwin_blur();
    }
}
