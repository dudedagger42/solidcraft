# Everything fixed and added (as of 2026-10-22 night, on 0.20.0)

Most of it is not merged upstream yet (rows marked **now in 0.20.0** are). All of it is in the all-in-one build (`solidcraft-0.20.0-all.zip`: upstream 0.20.0, its release commit, plus everything below).

## Bug fixes

| # | Fix | Where |
|---|---|---|
| 1 | The same constraint picked twice adds nothing (Parallel again on the same lines no longer stacks a copy) | #4392 |
| 2 | A constraint the sketch already implies is refused (no more over-constraining from the app) | #4392 |
| 3 | An arc's radius text defaults to the middle of the arc | #4392 |
| 4 | Offset keeps a rectangle drawn line by line together (coincident corners chain) | #4392 |
| 5 | Edges seen through see-through bodies can be clicked | #4392 |
| 6 | A drag (sketch point, 3D handle, dimension) keeps following the pointer over a floating card | #4392 |
| 7 | A dialog that wants a body (Combine, Airflow) lights the whole hovered body, not one face | #4392 |
| 8 | Move on a moved or turned component (imported part set upright) goes the way it's shown | #4455 |
| 9 | Align between parts placed in different frames: no more landing at right angles; snaps mapped too | #4455 |
| 10 | Extrude preview on a turned component is drawn on the face, not at right angles | #4455 |
| 11 | Move's triad and default pivot sit on the part (were off in the part's own frame) | #4546 |
| 12 | Move's typed lengths ("93.575 mm") turn with the part's frame: Rotate about a hole works, drags go the right way | #4546 |
| 13 | No seam line down imported hole walls; split hole halves count as one surface (#1468) | #4546 |
| 14 | Align by rims works on holes whose circles are split in halves | #4546 |
| 15 | Parts library: clicking or right-clicking a card no longer inserts it | #4432 |
| 16 | M outside a sketch no longer opens a new sketch (broke a component-move shortcut) | #4432 |
| 17 | Simulation: Speed/Pressure switch takes its clicks (two choice rows shared ids) | #4356 |
| 18 | Simulation: streamlines draw over the section and the bodies (were hidden inside) | #4356 |
| 19 | Simulation: the air no longer phases through thin printed walls (walls sealed, cells sized to walls) | #4356 |
| 20 | Chamfer over a joined pin: 14 of 15 sizes now build closed (was 10) | #4288 (draft; kernel-level, left as is) |
| 21 | Browser tree hover stays fast on a big nested STEP (Velocity-type files): raykholo's fix for our issue #3892 | upstream #3894 (not ours; included in the build) |
| 22 | Measuring the whole design (Claude CAD's `inspect_design`) no longer freezes the window until Windows closes it: it runs on a copy in the background (#4604) | #4614 |
| 23 | That measure is ~3.5x faster on big assemblies (56 s to 16-23 s on the 3,118-body stand-in): raykholo's measure speed-up | upstream #3907 (not ours; included in the build) |
| 24 | Parts library panel uses 0.19's UI kit controls (0.19's style check) | #4432 |
| 25 | Repeated parts (fasteners, brackets) drawn from one copy of their triangles; big meshes share vertices; copies out of view aren't drawn: oursland's viewport speed-ups | **now in 0.20.0** (#4647, with #4617 and #4638) |
| 26 | Move on two copies of one part (both selected) moves both the same way, not each its own way | #4849 |
| 27 | The part keeps up with the Move arrow while it's dragged (the preview draws the moved triangles instead of rebuilding the part) | #4912 |
| 28 | Move's value box follows the arrow being dragged (was always X): DuBearte's fix | upstream #4788 (not ours; included in the build) |
| 29 | Dragging one end of a tangent line leaves its other end where it is (it used to "go flying") | #4925 |
| 30 | Project no longer adds a second copy of a face border or edge the sketch already shows (the extra points and lines on rounded corners) | #4929 |
| 31 | Sketch Point refuses a snap it can't read instead of making nothing | #4745 |
| 32 | Orbit about the cursor over empty space turns about the part in view (a part far from the origin no longer flies off screen) | #4947 |
| 33 | MidPoint with a line's end point and another line: the point goes to the middle, the line stays | #5011 |
| 34 | Coincident on two circles (or arcs) lays the second on the first | #5038 |
| 35 | Thread: M7, M33, M39, M3.5 and the other ISO sizes are known; a Ø7 shaft gets M7 (was "M6 does not fit") | #5128 |
| 36 | A cut made while a body is hidden leaves that body alone | #5148 |
| 37 | A drop-down row's ⋯ menu opens over the drop-down instead of closing it (so Pin to Toolbar works from it) | test builds only |

## New features and quality of life

| # | What | Where |
|---|---|---|
| 1 | Simulation workspace: Airflow studies on the GPU (fans with curves, sections, streamlines, Goal Seek, smooth/rough finish, sized to the GPU) | #4356 |
| 2 | Revolve about a cylinder/cone/torus face or a circular edge | #4432 |
| 3 | A round face or circular edge as the axis in Revolve, Circular Pattern, Plane at Angle, Move → Rotate, Surface Revolve, following the part when it changes; a circle's centre as a point | upstream #4610 by froglemonade2 (replaces ours); round faces as a direction in Rectangular Pattern, Sweep, Ruled Surface stay ours (#4546) |
| 4 | Plane at Angle about a round face or circular edge | now from #4610 (our jeremymiribung-blip/solidcraft#1 on #2180 isn't needed for the build) |
| 5 | 3D sketch: points leave the plane; Move/Copy with an X/Y/Z triad | #4432 |
| 6 | Parts library panel: folder of designs and parts, drag into the view; double-click opens; right-click Open/Insert | #4432 |
| 7 | Sketch keys separate from model keys; M = Sketch Move inside a sketch, Move outside | #4432 |
| 8 | Move pivot, From or To on a circle = its centre; on a hole wall = its axis | #4546 |
| 9 | Align face onto face moves along the normal only ("Line Up Points Too" for the old way) | #4546 |
| 10 | Align rim onto rim lines up the circles' centres | #4546 |
| 11 | English in US spelling (center, color, gray, toward); English (UK) keeps UK | #4546 |
| 12 | ⋯ menu on drop-down rows (Pin, Change Keyboard Shortcut…) | test builds only, held back until finished |
| 13 | Extrude by a distance: click a body corner and it extrudes to it (To Object turns on by itself) | #4939 |
| 14 | Construction planes: drag their edges and corners to resize them; Reset Size in their menu | #4996 |
| 15 | Sketch: hovering a circle's edge shows its centre and makes it easy to click | #5043 |
| 16 | Circular Pattern and Revolve about a construction axis | **now in 0.20.0** (#3796) |

## Elsewhere

- FieldCraft #668: Smagorinsky turbulence model on the CPU (draft, invited by the maintainer; claims #663).
- Issues we filed: SolidCraft #3892 (slow STEP browser), #4055 (chamfer over a pin); FieldCraft #663.

## Not in the build

- Plane at Angle round axes (#2180 and our PR on it): that branch predates main's rework of the dialog and doesn't merge.

## Known, not fixed yet

- Join extrude with the design (top level) active on a body inside a moved component builds in the wrong place: activate the part's component first.
- Split Body / Delete Face on the BasicDuct (also on plain 0.18.2).
- Align has no 90° rotate option yet.
- Hovering a Rigid Group in the History bar on a big assembly (Velocity) stalls up to ~0.8 s while it lights every body in the group (#3894, included, fixes the browser tree, not this): next on the list.
- Importing a .solidcraft design can hang (reported 2026-10-22): need the file or steps.
- Move on a body and its Move-copy going two ways: not reproduced yet.
- With the XZ/YZ origin plane shown, a click on a face behind it in Project picks the plane.
