# Airflow solver validation (this fork)

How accurate the Airflow studies in this fork's builds are, measured against flows whose answers
are known. **Short version: laminar duct flow is accurate to about 1–2 % at 16–24 cells across a
passage; turbulent duct flow is not yet: pressure drops read 35–65 % high.** Treat turbulent
results (most real ducts and fans) as qualitative until that is fixed.

Measured 2026-10-24 on an RTX 4070 Laptop GPU, single precision (CPU double precision agrees to
the digits shown). The study is `crates/gpu/tests/flow_validation.rs` on branch `fan-rotor`:
`cargo test -p solidcraft-gpu --release --test flow_validation -- --ignored --nocapture`.

## Laminar flow (Re 50): validated

Fully developed flow in a straight bore 12 diameters long, measured between 50 % and 90 % of its
length. "Cells across" is the bore's diameter (or side) in cells. Errors against the exact
solutions: Hagen–Poiseuille for the round pipe, Shah & London (1978) for the square duct
(f·Re = 56.91, peak 2.096 × the mean).

| Case | Cells across | Pressure gradient | Peak speed | Velocity profile (L2) |
|---|---|---|---|---|
| Round pipe | 8 | +1.3 % | −4.1 % | 7.8 % |
| Round pipe | 16 | **−0.2 %** | −2.4 % | 3.7 % |
| Round pipe | 24 | **−0.7 %** | −2.2 % | 2.8 % |
| Square duct | 8 | +6.5 % | +2.2 % | 10.8 % |
| Square duct | 16 | +3.5 % | −0.9 % | 4.8 % |
| Square duct | 24 | **+1.6 %** | −2.2 % | 3.0 % |

Errors are against the flow actually in the bore (mass conserved along it to 0.1 %). The flow an
inlet delivers is within about 2–5 % of the one set; at this low Reynolds number part of that is
the lattice's compressibility (it carries pressure as density), which is far smaller at the
Reynolds numbers of real ducts.

**Recommendation:** 16 cells across the narrowest passage for ±4 %, 24 for ±2 %. The automatic
cell size today is 8 cells across: fine for a first look, ±5–10 %.

## Turbulent flow (Re 5000): not yet reliable

A round pipe 40 diameters long at Re 5000 (an 8 mm bore at 9.4 m/s), pressure gradient over its
last third against Blasius' friction factor (f = 0.3164 Re^-¼).

| Cells across | Smagorinsky 0.15 (default) | 0.10 | 0 (no model) |
|---|---|---|---|
| 16 | +53 % | +34 % | +4 %, then unstable |
| 24 | +64 % | unstable | unstable |

The flow is also the wrong shape (a flat core that runs no faster than the mean, where real
turbulent pipe flow peaks near 1.22 × the mean). At this Reynolds number the lattice runs a
relaxation time of 0.5003–0.5008, very close to its stability limit; the turbulence model keeps
it stable at the cost of too much mixing near the walls. Fixing it needs a more robust collision
operator (a cumulant or entropic one), a wall model, and a fully developed turbulent benchmark
(a periodic channel) rather than this developing pipe. That is the next piece of work.

## Fixed along the way (in the next all-in-one build)

- **Inlets delivered too little air.** A round inlet 8 cells across keeps only about two thirds
  of its area as whole cells; the set speed blew through those alone, so a study got **32 %
  less flow than asked** (47 % at 6 cells). Inlets now deliver the speed times the patch's true
  area.
- **The collision operator** is now the recursive regularized one (third-order moments kept):
  it fixed a wrong steady flow in the turbulent pipe and cut its error from +67 % to +53 %;
  laminar results are unchanged.
- **GPU work is sized by time** (about 0.1 s per submission). A fixed 32 steps per submission
  took several seconds on large grids or a shared GPU and could trip Windows' GPU watchdog.

## Not yet tested

Flow past bodies (drag, lift, vortex shedding), heat transfer, fans, and comparisons with
OpenFOAM on the same geometry.
