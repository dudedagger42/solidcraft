# SolidCraft looks as plugins

Gabe's all-in-one looks, moved from app code to `shaders@0.1` plugins (RFC 0015, amendment 1).
Each folder is one plugin; each `.zip` installs from Preferences › Plugins › Install plugin…, then
Preferences › Interface › Background effect picks it.

| Plugin | What it draws | Was in the all-in-one as |
|---|---|---|
| `galaxy` (360 Galaxy) | A wrap-around galaxy that turns with the view | Background picture › Use the galaxy (`all/looks` a4faf814f) |
| `rain` (Rain on the Window) | Rain on a frosted window over a city at night, animated | Background picture › Use the rain (`all/looks` 34e6beb5b) |

Not here yet:

- **Liquid Craft / Liquid Craft Dark** (glass panels over the model). The glass is a panel
  material, not a background: it needs a backdrop of the view that panels refract, which
  `shaders@0.1` does not offer. RFC 0015 amendment 1 lists `panel` materials as the next shader
  kind. Until then Liquid Craft stays app code on `all/liquid-craft`.

Differences from the all-in-one:

- The rain's drops are procedural (each column clings, then slips; small beads come and go). The
  all-in-one simulates drops on the CPU so they land, merge and remember their trails; a 0.1
  background keeps no state between frames.
- The picture is behind the model only; glass panels don't see it (no glass in 0.1).

Pictures: `galaxy/galaxy.jpg` and `rain/city.jpg` were made for Gabe by Claude (the all-in-one's
`assets/branding/space-background.jpg` and `rain-city.jpg`). They aren't derived from third-party
work and carry no third-party licence (Gabe, 2026-11-17), so they may ship in the plugins and the
public zip. `own-picture/default.png` is generated.

Build the zips with `python build.py`.
