#### WR-17356. `SmLSIGlobalSolver::LocalSolve` finds one of two separated roots in a leaf
- **Severity:** Wrong result
- **Area:** Intersection
- **Location:** `source/SMLib/src/SmSurface.cpp:18075` (`SmLSIGlobalSolver::LocalSolve`) at v1.0.1 (`3fd5b95`), as the unit's fixture cites it (`fixtures/surface.surface_p10/cases.json`, `upstream_ref`). In Rust: `crates/solidcore/src/surface/surface/p10.rs`, `LsiGlobalSolver::local_solve_branch`.
- **What happens:** A line leaf seeds one Newton guess at the middle of its interval and looks for further roots only through the near-tangent neighbour, and only when the polar box allows more. A second root well apart in the same leaf is lost.
- **Trigger:** A ray crossing a convex rational patch twice inside one cache leaf (#17356: face 34, roots at t 43.03 and 54.118).
- **How found:** #17356 probes.
- **Confirmed:** no (no C++ run). The port follows the pinned C++ (`3fd5b95`) here, and on the port the unit test `line_crossing_a_rational_patch_twice_gets_both_roots` gets one root of two on main.
- **Rust port:** fixed (deviation): after a hit the line crosses (classified `Crossing`), both sides of it are searched, each on its own part of the line interval, within `MAX_GUESSES`. A line lying in the surface or touching it is not searched further, so a coincident ray keeps upstream's result. Goldens: see the PR (`surface.surface_p10`'s `adv_lsi_ray_in_plane` moved when the sides of an in-plane hit were searched; with the crossing gate it should not).
