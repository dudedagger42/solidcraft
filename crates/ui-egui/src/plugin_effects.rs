//! Effects a plugin offers the host to draw (capability `effects@0.1`, `docs/plugin-effects.md`).
//!
//! A look is on while its plugin is turned on: the switch on the plugin's card in Preferences ›
//! Plugins. Its settings are on the same card. The kinds the host draws: `hue-wave` (the RGB wave
//! over the ribbon, its button blocks and the panels), `galaxy` (the 360 galaxy behind the model)
//! and `rain` (rain on a frosted window over a city at night).

use std::collections::BTreeSet;

use crate::SolveApp;
use crate::icons::{Wave, WaveSet};

/// The effect kind the host draws as the RGB wave.
pub(crate) const HUE_WAVE: &str = "hue-wave";
/// The 360 galaxy behind the model.
pub(crate) const GALAXY: &str = "galaxy";
/// Rain on a frosted window behind the model.
pub(crate) const RAIN: &str = "rain";

/// The wave as the ribbon and panels draw it this frame. Off unless a plugin offering it is on.
pub(crate) fn wave_set(app: &mut SolveApp) -> WaveSet {
    let on = active(app, HUE_WAVE);
    let p = &app.preferences;
    WaveSet {
        on,
        icons: p.ui_effect_icons,
        blocks: p.ui_effect_buttons,
        panels: p.ui_effect_panels,
        wave: Wave { length: p.ui_effect_length.max(1.0), speed: p.ui_effect_speed },
    }
}

/// Whether a plugin that is on offers an effect of `kind`. Read with the plugin themes, only when
/// the plugins change ([`crate::plugin_themes::sync`]).
pub(crate) fn active(app: &SolveApp, kind: &str) -> bool {
    app.plugin_effects.kinds.contains(kind)
}

/// The kinds the plugins that are on offer.
pub(crate) fn set_active(app: &mut SolveApp, kinds: BTreeSet<String>) {
    app.plugin_effects.kinds = kinds;
}

/// What the state keeps between frames: the kinds on.
#[derive(Default)]
pub(crate) struct State {
    kinds: BTreeSet<String>,
}
