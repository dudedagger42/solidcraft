#### WR-17356. `SmLSIGlobalSolver::LocalSolve` finds one of two separated roots in a leaf
- **Severity:** Wrong result
- **Area:** Intersection
- **Location:** `source/SMLib/src/SmSurface.cpp`, `SmLSIGlobalSolver::LocalSolve`, at v1.0.1 (`3fd5b95`); it sits with `SmLSIGlobalSolver::BranchMayContainAnswers` (`:17963`). The exact line is not given here: the C++ source is not on the machine this was written on. In Rust: `crates/solidcore/src/surface/surface/p10.rs`, `LsiGlobalSolver::local_solve_branch`.
- **What happens:** A line leaf seeds one Newton guess at the middle of its interval and looks for further roots only through the near-tangent neighbour, and only when the polar box allows more. A second root well apart in the same leaf is lost.
- **Trigger:** A ray crossing a convex rational patch twice inside one cache leaf (#17356: face 34, roots at t 43.03 and 54.118).
- **How found:** #17356 probes.
- **Confirmed:** no (no C++ run). The port follows the pinned C++ (`3fd5b95`) here, and on the port the unit test `line_crossing_a_rational_patch_twice_gets_both_roots` gets one root of two on main.
- **Rust port:** fixed (deviation): after each root found in a leaf, both sides of it are searched, each on its own part of the line interval, within `MAX_GUESSES`. Goldens: `surface.surface_p10`'s were not run on that machine (its build queue has no `cargo run`), so whether a case moves is left to CI's goldens job. A case moves only where a line crosses one leaf twice, well apart.
