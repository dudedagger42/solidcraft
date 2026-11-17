#### WR-17356. `SmLSIGlobalSolver::LocalSolve` finds one of two separated roots in a leaf
- **Severity:** Wrong result
- **Area:** Intersection
- **Location:** `SmLSIGlobalSolver::LocalSolve` (the upstream leaf solve that `LsiGlobalSolver::local_solve_branch` in `solidcore/crates/solidcore/src/surface/surface/p10.rs` ports, named there as `SmLSIGlobalSolver::LocalSolve`).
- **What happens:** A line leaf seeds one Newton guess at the middle of its interval and looks for further roots only through the near-tangent neighbour, and only when the polar box allows more. A second root well apart in the same leaf is lost.
- **Trigger:** A ray crossing a convex rational patch twice inside one cache leaf (#17356: face 34, roots at t 43.03 and 54.118).
- **How found:** #17356 probes.
- **Confirmed:** yes (unit test `line_crossing_a_rational_patch_twice_gets_both_roots`).
- **Rust port:** Fixed, with each side of a found root searched on its own part of the interval.
