# SolidCraft 0.18.4 test build (Windows x64)

Unzip anywhere and run `solidcraft.exe`. Nothing to install. `solidcraft-cli.exe` is the command-line tool, and `mock-duct\` has the part-cooling duct study.

This is upstream `main` (0.18.4) with everything we have finished:

| PR | What | Try |
|---|---|---|
| #4356 Simulation | Airflow studies on the GPU; smooth/rough finish; Speed/Pressure switch; streamlines drawn over the section | Simulation workspace → Airflow on a duct; switch Speed/Pressure; tick Streamlines |
| #4392 Bug fixes | duplicate and implied constraints refused; arc radius text; rectangle offset; picking edges through see-through bodies; drags over floating cards; the whole body lights when a dialog wants a body | Combine or Airflow: hover a body; sketch: constrain an already-horizontal line |
| #4432 QOL | revolve about a round face or circular edge; 3D sketch points off the plane and Move/Copy with X/Y/Z; parts library on the left; sketch keys apart from model keys (M = Sketch Move in a sketch) | |
| #4455 Round 2 | Move, Align and extrude previews on moved or turned components (imported parts set upright) | the DrillPress plate: Move up; Align two plates; extrude a face with its component active |
| round 3 (not a PR yet) | a cylinder/cone/torus face or a circular edge works as the axis in Circular Pattern, Move → Rotate, Plane at Angle and Surface Revolve, and as the direction in Rectangular Pattern, Sweep and Ruled Surface | Circular Pattern: pick a hole's wall or rim as the Axis |
| chamfer draft (#4288) | the partial chamfer-over-a-pin improvement | |

Not in it: the ⋯ menu on drop-down rows (held back).

Known, not fixed: with the design (top level) active, a Join extrude on a face of a body inside a moved component builds in the wrong place. Activate the part's component first. Split Body and Delete Face fail on the BasicDuct file (also on plain 0.18.2).
