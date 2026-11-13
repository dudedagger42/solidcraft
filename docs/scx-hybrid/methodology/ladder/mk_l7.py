import json, os, subprocess, sys
# Builds every L7 reference state through scx on the SolidCraft window at SC_PORT (default 7882) and writes
# ../bench/specsL7.json. usage: python mk_l7.py   (env HY_SERVER: the scx server; the original used v15)
HERE = os.path.dirname(os.path.abspath(__file__))
BENCH = os.path.join(HERE, "..", "bench")
sys.path.insert(0, BENCH); os.environ.setdefault("SC_PORT", "7882")
import bench as B
R = os.path.join(HERE, "ref7")
def rd(f): return open(os.path.join(R, f), encoding="utf-8").read()
def build(f):
    subprocess.run([sys.executable, os.path.join(HERE, "ref.py"), os.environ["SC_PORT"], os.path.join(R, f)], check=True, capture_output=True)
    return [{"name": b["name"], "volume": round(b["volume"], 4), "bbox": [[round(v, 4) for v in p] for p in b["bbox"]]} for b in B.check("sc")]
T = {
 "L7_duct_elbow_iter": ("duct_start.py", "duct", [
  "Add a 45-degree elbow named Elbow as a separate body: a bent tube with the same 44 mm outside and 40 mm inside diameter as the Outlet, starting at the top of the Outlet and bending from vertical towards +X through 45 degrees, with a 40 mm centreline radius.",
  "Add a straight pipe named Pipe as a separate body: the same 44/40 mm section, 50 mm long, continuing straight on from the end of the elbow.",
  "Make the duct 20 mm taller (80 mm instead of 60), with the same bottom and top sections. The Outlet, Elbow and Pipe move up with it so everything stays connected.",
  "Add a flange named PipeFlange to the free end of the pipe as a separate body: a disc 70 mm in diameter and 6 mm thick with a 44 mm bore so it fits over the pipe, square to the pipe, its outer face flush with the pipe's end."]),
 "L7_impeller_iter": ("imp_start.py", "imp", [
  "The design holds Impeller: a 120 mm backplate disc 4 mm thick on the XY plane, centred on the Z axis, with a 30 mm hub on the axis rising to z = 20 and an 8 mm bore through. Add eight straight blades to it, fused into Impeller, evenly spaced around the axis with the first along +X. Each blade is a loft standing on the backplate, from a 3 mm thick, 16 mm tall rectangle at 14 mm from the axis (inside the hub) to a 3 mm thick, 8 mm tall rectangle at 58 mm from the axis. Both rectangles' bottom edges sit on the backplate's top face and are centred on the blade's centreline.",
  "Add a 5 mm balancing hole through the backplate midway between each pair of neighbouring blades, on a 90 mm diameter circle.",
  "Make the hub 10 mm taller (top at z = 30), with the bore still running right through. Then add a 3 mm set-screw hole drilled from the outside of the hub along -Y on the +Y side, 5 mm below the new hub top, into the bore but not out the other side.",
  "Add a shaft named Shaft as a separate body: 8 mm in diameter and 60 mm long, in the bore, sticking out 20 mm below the backplate (z -20 to 40).",
  "Change the impeller to six blades instead of eight (same blade shape, evenly spaced, the first still along +X), and move the balancing holes so there is again one midway between each pair of neighbouring blades (six holes, same circle)."]),
 "L7_handle_plate_iter": ("hdl_start.py", "hdl", [
  "The design holds Base: a plate 100 x 100 mm, 8 mm thick, spanning x 0..100, y 0..100, z 0..8. Add a carrying handle named Handle as a separate body: a 10 mm round bar that rises straight up from the plate's top face at (20, 50), bends over through a quarter circle of 15 mm centreline radius, runs straight along X at a centreline height of z = 60, bends down through another 15 mm quarter circle and comes straight down to the plate's top face at (80, 50). The bar's ends are flat and sit on the plate.",
  "Add a counterbored hole near each corner of the plate, centred 10 mm in from both edges: 6.6 mm through, with an 11 mm counterbore 4 mm deep from the top.",
  "Add a second handle identical to the first but turned 90 degrees about the plate's vertical centre axis, so the two cross at the top. Fuse it into Handle.",
  "Lengthen the plate by 40 mm at its +X end. Move the handles so they stay centred on the plate, and move the corner holes so each is again 10 mm in from both edges.",
  "Round the plate's four vertical corner edges with a 10 mm radius."]),
}
out = {}
for task, (start, stem, prompts) in T.items():
    steps = []
    for i, p in enumerate(prompts, 1):
        steps.append({"prompt": p, "bodies": build(f"{stem}_{i}.py")})
        print(task, i, [(b["name"], b["volume"]) for b in steps[-1]["bodies"]])
    out[task] = {"setup": rd(start), "steps": steps}
json.dump(out, open(os.path.join(BENCH, "specsL7.json"), "w", encoding="utf-8", newline="\n"), indent=1)
