/* acrylic_style.rs
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

use gtk::{gio, prelude::*};

pub const SETTING_KEYS: [&str; 5] = [
    "acrylic-tint-opacity",
    "acrylic-noise-opacity",
    "acrylic-highlight-opacity",
    "acrylic-shadow-opacity",
    "acrylic-divider-opacity",
];

fn opacity(settings: &gio::Settings, key: &str) -> f64 {
    f64::from(settings.int(key).clamp(0, 100)) / 100.0
}

fn update_provider(provider: &gtk::CssProvider, settings: &gio::Settings) {
    let tint = opacity(settings, SETTING_KEYS[0]);
    let noise = opacity(settings, SETTING_KEYS[1]);
    let highlight = opacity(settings, SETTING_KEYS[2]);
    let shadow = opacity(settings, SETTING_KEYS[3]);
    let divider = opacity(settings, SETTING_KEYS[4]);

    provider.load_from_string(&format!(
        r#"
.mission-center-window.acrylic-enabled .mission-center-shell .sidebar-pane,
.mission-center-window.acrylic-enabled .mission-center-shell .overlay-pane {{
    background-color: alpha(@theme_bg_color, {tint:.2});
    background-image:
        cross-fade({noise_percent:.0}% url("resource:///io/missioncenter/MissionCenter/acrylic-noise.svg"), image(transparent)),
        linear-gradient(
            135deg,
            alpha(white, {highlight:.2}),
            alpha(white, {highlight_mid:.3}) 42%,
            alpha(black, {shadow:.2})
        );
    background-repeat: repeat, no-repeat;
    background-size: 96px 96px, cover;
    box-shadow: inset -1px 0 alpha(@theme_fg_color, {divider:.2});
}}
"#,
        highlight_mid = highlight * 0.28,
        noise_percent = noise * 100.0,
    ));
}

pub fn install(display: &gtk::gdk::Display, settings: &gio::Settings) {
    let provider = gtk::CssProvider::new();
    update_provider(&provider, settings);
    gtk::style_context_add_provider_for_display(
        display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
    );

    for key in SETTING_KEYS {
        let provider = provider.clone();
        settings.connect_changed(Some(key), move |settings, _| {
            update_provider(&provider, settings);
        });
    }
}
