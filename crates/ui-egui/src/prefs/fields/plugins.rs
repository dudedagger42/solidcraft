//! Stored preferences on the Plugins page (`PAGE_PLUGINS`).
//!
//! Each entry is one field of `Prefs`, in `preference_settings!`'s form:
//! `field: Type = default, PAGE[, accept(range)][ => control(...)];`. Add a new preference
//! to the file of the page whose Defaults resets it, in alphabetical order by field name, so
//! two new preferences rarely land on the same line.

macro_rules! plugins_preferences {
    ($next:ident $(, $rest:ident)* ; $($fields:tt)*) => {
        $next! { $($rest),* ; $($fields)*
            /// A plugin's background effect behind the model, `plugin-id/shader-id` (`shaders@0.1`);
            /// empty (the default) for none. Chosen on its plugin's card.
            background_shader: String = String::new(), PAGE_PLUGINS;
            /// The picture each chosen texture of a background effect shows, `plugin-id/shader-id/<texture
            /// index>` to an absolute file path (`choose` in the manifest). Absent: the shipped `file`.
            background_shader_pictures: BTreeMap<String, String> = BTreeMap::new(), PAGE_PLUGINS;
            /// Plugin ids turned off (Preferences › Plugins). An installed plugin whose id is absent is on.
            disabled_plugins: std::collections::BTreeSet<String> = std::collections::BTreeSet::new(), PAGE_PLUGINS;
            /// A glass theme's blur, as a share of the theme's own: 0 (clear) to 2.
            glass_blur: f32 = 0.85, PAGE_PLUGINS, accept(0.0..=2.0);
            /// A glass plugin's dark theme while it is on (Liquid Craft Dark); off: its light one.
            glass_dark: bool = true, PAGE_PLUGINS;
            /// A glass theme's rim reach, as a share of the theme's own: 0 (the rim only magnifies what
            /// is under the glass) to 1 (the theme's: the backdrop up to the bend past the edge).
            glass_reach: f32 = 0.5, PAGE_PLUGINS, accept(0.0..=1.0);
            /// A glass theme's rim bend (refraction), as a share of the theme's own: 0 (flat) to 2.
            glass_refraction: f32 = 1.0, PAGE_PLUGINS, accept(0.0..=2.0);
            /// A glass theme's see-through, as a share of the theme's own: 0 (the panels' colour) to 2.
            glass_see_through: f32 = 0.85, PAGE_PLUGINS, accept(0.0..=2.0);
            /// A glass theme's glass tinted with the accent; off: clear glass.
            glass_tint_accent: bool = false, PAGE_PLUGINS;
            /// User ceilings, not automatic increases to a package's approved execution budget.
            plugin_max_fuel: u64 = solidcraft_engine::plugin::HostLimits::default().max_fuel, PAGE_PLUGINS;
            plugin_max_memory_mib: usize = solidcraft_engine::plugin::HostLimits::default().max_memory_mib, PAGE_PLUGINS;
            /// Remember a moved plugin command window's position for the current design, until the
            /// application closes (#18684). Off keeps the previously centred reopen behaviour.
            plugin_remember_dialog_position: bool = false, PAGE_PLUGINS;
            /// How hard it rains while a rain plugin is on: 0 (no new drops) to 1.
            rain_amount: f32 = 0.6, PAGE_PLUGINS, accept(0.0..=1.0) => slider("prefs.rain_amount", 0.0..=1.0, 0.05, |v| format!("{:.0} %", v * 100.0), "prefs:rain_amount");
            /// How bright the rain's picture is: 0 (nearly black) to 1 (the picture as it is).
            rain_brightness: f32 = 1.0, PAGE_PLUGINS, accept(0.0..=1.0) => slider("prefs.rain_brightness", 0.0..=1.0, 0.05, |v| format!("{:.0} %", v * 100.0), "prefs:rain_brightness");
            /// A plugin's effect the host draws (`effects@0.1`), `plugin-id/effect-id`; empty (the default)
            /// for none. Chosen on its plugin's card.
            ui_effect: String = String::new(), PAGE_PLUGINS;
            /// The same wave tints each ribbon button's block at low alpha.
            ui_effect_buttons: bool = false, PAGE_PLUGINS;
            /// The RGB wave, while its plugin is on: the ribbon's command icons take its hue.
            ui_effect_icons: bool = true, PAGE_PLUGINS;
            /// How long one colour cycle of the wave is on screen (points).
            ui_effect_length: f32 = 2400.0, PAGE_PLUGINS, accept(300.0..=8000.0) => slider("prefs.ui_effect_length", 300.0..=8000.0, 100.0, |v| format!("{v:.0} pt"), "prefs:ui_effect_length");
            /// The same wave over the UI's panels and bars.
            ui_effect_panels: bool = true, PAGE_PLUGINS;
            /// How fast the wave travels (points a second).
            ui_effect_speed: f32 = 150.0, PAGE_PLUGINS, accept(10.0..=1500.0) => slider("prefs.ui_effect_speed", 10.0..=1500.0, 10.0, |v| format!("{v:.0} pt/s"), "prefs:ui_effect_speed");
        }
    };
}
