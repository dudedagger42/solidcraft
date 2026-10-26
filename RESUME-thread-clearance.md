# Thread clearance: where it stands (2026-10-26)

Done: field + pending format note, `solid.thread {clearance}`, dialog tiles (Wider default /
Narrower) with pictures, the amount, and a yellow warning icon (hover explains) when the
clearance trims the tooth tips (`thread_clearance_trims_teeth`). Geometry: flanks and root
offset; a groove wider than 0.49 P is held there (tips trimmed to a ridge); a narrowed groove
ends where its flanks meet. Engine thread tests 24/24. UI thread tests 62/64: the 2 failures
(`contribution_metric_designation_rendering…`, `contribution_thread_catalogue…`) fail the same
on upstream main a6354dfe.

Left: a UI test for the tiles and warning, screenshots, file the issue + /claim, draft PR,
cherry-pick into Gabe's build.
