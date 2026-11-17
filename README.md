# SolidCraft looks as plugins

Gabe's all-in-one looks as plugins. The app draws each look (`effects@0.1`, kinds `galaxy`, `rain`,
`hue-wave`), and the plugin's switch on Preferences › Plugins turns it on and off. Its settings are
on the same card. Install each `.zip` from Preferences › Plugins › Install plugin….

| Plugin | What it is | Settings on its card |
|---|---|---|
| `galaxy` (360 Galaxy) | A wrap-around galaxy behind the model that turns with the view | none |
| `rain` (Rain on the Window) | Rain on a frosted window over a city at night: drops land, merge and run off; none fades | Rain amount |
| `rgb-wave` | The RGB wave over the ribbon, its buttons and the panels | icons, buttons, panels, length, speed |
| `liquid-craft` | The Liquid Craft and Liquid Craft Dark glass themes | glass blur, see-through, refraction, reach, accent tint |

Solid Glass is a plain theme file in the app (`crates/look/themes/solid-glass.json`), not a plugin.
Your own background picture is Preferences › Appearance › Background picture, also in the app.

Pictures: the app's `assets/branding/space-background.jpg` and `rain-city.jpg` were made for Gabe
by Claude. They aren't derived from third-party work and carry no third-party licence (Gabe,
2026-11-17).

Build the zips with `python build.py`.
