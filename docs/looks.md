# Hand-made looks

The looks made for Gabe's builds: themes, backgrounds and effects that are not upstream. All of
them are on the fork's `all/looks` branch (`dudedagger42/solidcraft`), which goes into every
all-in-one build (`all-<tag>`). Some started on a topic branch of their own, listed here.

Preferences means Preferences > Interface (the Appearance page in older builds). Every setting
here is also a `ui.prefs` key of the control protocol, given in `code`.

| Look | Where it started | Commits | Main files | Turn it on |
|---|---|---|---|---|
| Solid Glass: a frosted dark theme, green accent | `all/solid-glass` | 0e4ed7282 | `assets/styles/ui/themes/solid-glass.json` | Theme: Solid Glass (`theme: "solid-glass"`) |
| Theme accent: any accent colour over a theme that accepts one | `all/theme-accent` | 951e507ef, 59c030c0e | `crates/ui-egui/src/theme.rs` (`accepts_accent`), `prefs/interface.rs` | Accent swatches or the colour field (`accent: "#rrggbb"`; `""` is the theme's own). The logo can follow it: Logo: Accent (`logo_accent: true`) |
| Liquid Craft and Liquid Craft Dark: see-through glass panels over the model, with lens edges and text that flips against the model | `all/liquid-craft` | 88008169c (egui-glass), 4486f55cd, d4157714c, 0f623c601, fbaeaca67, cd15a66b1, 8aebf5f59, c24f2436e, e70688b56 | `vendor/egui-glass/` (glass shader `glass.wgsl`, `scene.rs`), `crates/ui-egui/src/glass.rs`, `assets/styles/ui/themes/liquid-craft*.json` | Theme: Liquid Craft or Liquid Craft Dark (`theme: "liquid-craft"`, `"liquid-craft-dark"`). Sliders: Glass blur, See-through, Edge bend, Edge reach (`glass_blur`, `glass_see_through`, `glass_refraction`, `glass_reach`); Glass tint (`glass_tint_accent`) |
| RGB wave: ribbon icons, button blocks and panels cycle through hue in a wave | `all/looks` | 43f750a74, eee0e8c9a, 5d66c4371, 3e3b1e152 | `crates/ui-egui/src/toolbar.rs`, `icons.rs` | RGB wave on toolbar icons / button backgrounds / UI panels (`rgb_wave`, `rgb_wave_background`, `rgb_wave_panels`); Length and Speed sliders (`rgb_wave_length`, `rgb_wave_speed`). The logo follows the wave too |
| 360 galaxy: a wrap-around picture behind the model that turns with the view, seen through the glass (it began as a flat star field, 2d7dcce6c) | `all/looks` | 2d7dcce6c, a4faf814f | `vendor/egui-glass/src/sky.rs`, `crates/ui-egui/src/gpu.rs` (`paint_space`), `assets/branding/space-background.jpg` | Background picture, then Use the galaxy (`space_background: true`, `background_image: ""`) |
| Your own picture: any image behind the model, flat or 360 (a 2:1 picture turns with the view) | `all/looks` | a4faf814f | as the galaxy | Background picture, then Choose… (`background_image: "<path>"`) |
| Rain on a frosted window over a city at night: drops land, stick, merge, slide faster as they grow and wipe trails in the fog | `all/looks` | 458122dda (first, drops drawn without memory), 34e6beb5b (simulated drops) | `vendor/egui-glass/src/rain.rs` (the drops), `vendor/egui-glass/src/sky.rs` (the shader), `crates/ui-egui/src/gpu.rs`, `assets/branding/rain-city.jpg` | Background picture, then Use the rain (`space_background: true`, `background_image: "builtin:rain"`) |

## Notes

- The background picture is drawn into a window-sized texture of its own (`sky.rs`), which the
  view shows and the glass themes copy, so glass panels see the same picture.
- The rain is the only look that animates by itself; it redraws about 30 times a second while the
  window has the focus, 10 times behind another window.
- A test copy of SolidCraft can try any of these without touching the saved preferences: start it
  with `SOLIDCRAFT_SETTINGS_DIR` and `SOLIDCRAFT_RECOVERY_DIR` set to scratch folders.
