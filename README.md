# SolidCraft 0.19.0 (overnight build, 2026-10-22): everything we have, in one build (Windows x64)

Unzip anywhere and run `solidcraft.exe`. `solidcraft-cli.exe` is the command-line tool; `mock-duct\` is the part-cooling duct study.

Upstream 0.19.0 plus every fix and feature we have that isn't merged yet: simulation, bug fixes, QOL, round 2, round 3, the chamfer draft, the ⋯ menu (test builds only), raykholo's #3894 (fast browser tree on big STEP assemblies, the fix for our #3892) and #3907 (faster measuring), our #4614 (measuring a whole big design no longer freezes the window), froglemonade2's #4610 (round axes that follow the part) and oursland's #4617/#4638/#4647 (faster drawing of big assemblies).

The full list, with which PR each item is in, is in `fixed-and-added.md`. If something looks wrong, check it there first: if it's listed as fixed, tell me; if not, we check whether plain `main` does it too.


## New in the overnight build

- **Faster B-spline geometry** (our change, held from upstream while kernel work is paused): booleans on lofted parts ~2.5x faster (the duct cut 171 s → 67 s), big Fusion STEP assemblies open ~4x faster. Results are bit-identical.
- froglemonade2's #4610 (round axes that follow the part), oursland's #4617/#4638/#4647 (fast drawing of big assemblies), DuBearte's #4611 (faster freeform joins).
- Tonight's fixes:
  - Point tool on a midpoint is held there (#4745);
  - STEP inserts through Claude CAD keep their placement (#4746);
  - `rollback -1` refused (#4749);
  - **mesh export Quality: Standard / Fine / Finest** in Save As Mesh (#4773).
- Not fixed yet: moving several copies of a repeated part with M sends them flying (#4768). Move those as components for now.
