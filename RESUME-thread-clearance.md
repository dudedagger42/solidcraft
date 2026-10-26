# Thread clearance: where it stands (2026-10-26)

Done (compiles): `FeatureKind::Thread.clearance: Option<Expr>` + `format/pending/thread-clearance.rs`;
`solid.thread {clearance}` (+ params doc); dialog: `Kind::Thread {clearance, narrower}`, Wider/Narrower
tiles with pictures (`dialogs_thread::clearance_rows`, default Wider), reopen (`thread_fit`), unit
conversion; groove growth + air-end cap in `thread_groove` (eval.rs).

Left: at printing clearances (0.1–0.2 mm on M6) the grown grooves meet before the tooth tips, so the
current cap check refuses them. Fix: crest radius `at = r0 ± (cap - w0)/tan30` (independent of r1):
- shaft: turn down to min(d/2, at): generalise `thread_turn` to a target radius;
- hole: bore the hole out to `at` over the threaded length (revolved ring cut, like thread_turn's ring);
then build the groove with `radius` = that crest radius. Then: tests in engine `thread_tests.rs`
(`clearance_widens_or_narrows…` expects 0.2 to work on shaft and hole), a UI test for the tiles,
screenshots, file the issue + /claim, draft PR, cherry-pick into all-0.22/0.23.
