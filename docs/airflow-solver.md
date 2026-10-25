# The Airflow solver: how it works and where it is going (this fork)

> **Not for engineering use.** Airflow studies in this fork are experimental. Use them to compare
> designs against each other, not for absolute numbers. Measured accuracy is in
> [airflow-validation.md](airflow-validation.md).

## How it works today

- **Method:** lattice Boltzmann on a D3Q19 lattice: a grid of cubic cells, each carrying the
  amounts of fluid moving in 19 directions. It runs on the GPU (WGSL through wgpu, single
  precision), with a CPU reference in double precision to check it against.
- **Collision:** regularized, now recursive regularized (third-order Hermite terms, Coreixas et
  al. 2017), which stays stable at the low viscosities air needs on coarse cells.
- **Turbulence:** large-eddy style (Smagorinsky, damped near walls by van Driest). It resolves
  the large eddies and models the small ones.
- **Walls:** interpolated (Bouzidi), so curved walls aren't stair-stepped.
- **Inlets, outlets and fans:** a velocity or flow-rate inlet, pressure outlets, and fan curves
  as body forces (Guo forcing).

### Fixed on 2026-10-24

| Problem | Effect | Now |
|---|---|---|
| Inlet rim cells were dropped | the inlet delivered up to 32 % too little flow | within about 3–5 % |
| The collision under-applied body forces | fans and driven flows pushed too weakly | the laminar check is within 2–3 % |
| A design opened in a new tab lost the GPU | it ran on the CPU, slowly, capped at 2 million cells | fixed in the 0.22 build |
| Long GPU submissions | could trip the Windows driver watchdog | work is sent in batches of about 0.1 s |

### Accuracy

- **Laminar flow: validated.** A pipe came out within 0.2–0.7 % of Hagen–Poiseuille and a square
  duct within 1.6 % of Shah & London, at 16–24 cells across.
- **Turbulent flow: not yet reliable.** Pressure drops read 35–65 % high. A
  fully turbulent benchmark pipe (Re_τ 180 against Eggels et al.'s DNS) drifted or slowly went
  laminar. The cause: duct-scale cells are far too coarse to resolve the thin layer of flow next
  to the walls, so the walls get the wrong friction.

So the solver is good for **comparing** designs (which duct flows more evenly or loses less),
not yet for absolute pressure drops or CFM in turbulent flow.

## Plan A: a fast RANS mode (in progress)

Solve the **average** flow instead of the eddies. Turbulence becomes an eddy viscosity from
Prandtl's mixing length (l = min(κy, 0.07 D), as Nikuradse measured). The wall layer is not
resolved; a law of the wall (Spalding 1961) gives the friction at the wall instead. Coarse cells
are then fine, and a run takes minutes, about what it takes today.

- **Done:** the model is in both the GPU and CPU solvers, off by default. On the benchmark pipe its
  eddy viscosity matches the model's own exact 1D answer within 1–4 %.
- **Next:** the cells next to walls still give too little friction, and the flow runs away toward
  laminar. The fix writes the law-of-the-wall shear straight into those cells' populations
  (Malaspinas & Sagaut 2014).
- **Then:**
  - validate across Reynolds numbers on the pipe;
  - check the losses of a sudden expansion, a contraction and a bend against handbook values;
  - set the automatic cell size from those results (likely 24–32 cells across the narrowest
    passage).
- **Target:** pressure drop and flow within 5–10 %. RANS stays off until it gets there.

## Plan B: a "precise" mode (next)

A cumulant collision on a D3Q27 lattice, resolving turbulence properly (LES). It's slower,
roughly 3–10 minutes or more per run, so it's meant for a final check, for example the last solve
of a goal-seek run. The study settings will get a Fast / Precise toggle.

## After that

- Flow plus heat: part-cooling ducts, then water blocks.
- Compressible effects are out of scope for now. The solver assumes low Mach number, which holds
  for ducts and fans but not for very high blade-tip speeds.
