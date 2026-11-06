import json, os, subprocess, sys
# Builds every L6 reference state through scx on the SolidCraft window at SC_PORT (default 7882) and writes
# ../bench/specsL6.json. usage: python mk_l6.py   (env HY_SERVER: the scx server; the original used v14)
HERE = os.path.dirname(os.path.abspath(__file__))
BENCH = os.path.join(HERE, "..", "bench")
sys.path.insert(0, BENCH); os.environ.setdefault("SC_PORT", "7882")
import bench as B
R = os.path.join(HERE, "ref6")
def rd(f): return open(os.path.join(R, f), encoding="utf-8").read()
def run(f, keep):
    subprocess.run([sys.executable, os.path.join(HERE, "ref.py"), os.environ["SC_PORT"], os.path.join(R, f)] + (["keep"] if keep else []), check=True, capture_output=True)
def bodies():
    return [{"name": b["name"], "volume": round(b["volume"], 4), "bbox": [[round(v, 4) for v in p] for p in b["bbox"]]} for b in B.check("sc")]
T = {
 "L6_gearbox_iter": ("gear_0.py", [("gear_1.py", "Add a second gear named Gear2 on ShaftB: a plain disc 40 mm in diameter and 10 mm thick, at the same position along its shaft as Gear is on ShaftA, with a 12 mm bore. Keep it a separate body."),
   ("gear_2.py", "Add a mounting foot at each short end of the housing, as part of the Housing body: a plate 4 mm thick, flush with the bottom of the housing, running the full 80 mm depth and sticking out 20 mm from the end wall. Each foot gets two 8 mm holes, centred across the foot's width, 25 mm either side of the housing's centre line."),
   ("gear_3.py", "Add six 8 mm lightening holes through Gear (the gear on ShaftA), evenly spaced on a 32 mm diameter circle around its axis, one of them straight above the shaft."),
   ("gear_4.py", "Make the lid 3 mm thicker, growing upwards (its underside stays on the housing). Its four corner holes still go right through, and add a 20 mm filler hole through the centre of the lid.")]),
 "L6_pulley_iter": ("pul_0.py", [("pul_1.py", "Cut a keyway along the full length of the pulley's bore: 6 mm wide, centred on the +X side of the bore, with its flat end 3 mm beyond the bore wall (at x = 13)."),
   ("pul_2.py", "Add a 5 mm set-screw hole, drilled from the outside of the hub along -Y on the +Y side, 7 mm above the bottom face, going into the bore but not out the other side."),
   ("pul_3.py", "Make the six holes in the web 10 mm in diameter instead of 8 mm, in the same positions."),
   ("pul_4.py", "Add a copy of the finished pulley as a separate body named Pulley2, stacked directly on top of the first one (its bottom face on the first one's top face).")]),
 "L6_motor_plate_iter": ("mp_0.py", [("mp_1.py", "Lengthen the plate by 60 mm at its x = 100 end, so it is 160 mm long. Every existing feature stays where it is, and the front chamfer runs the full new length."),
   ("mp_2.py", "Add a second boss set identical to the existing one (40 mm boss 6 mm tall, 22 mm bore through, four 5 mm through holes on a 31 mm circle at 45, 135, 225 and 315 degrees), centred at x = 120, same y as the first."),
   ("mp_3.py", "The 8 mm back hole and the 4 mm chamfer hole that were nearest the old x = 100 end should now sit the same distances from the new end (12 mm and 25 mm): move them. Then add a stiffening rib on the top face along the back edge: 4 mm thick, 20 mm tall, flush with the back face, running from x = 20 to x = 140."),
   ("mp_4.py", "Round the two vertical corner edges at the back of the plate (at x = 0 and x = 160, y = 80) with a 10 mm radius.")]),
}
out = {}
for task, (start, steps) in T.items():
    run(start, False)
    st = []
    for f, prompt in steps:
        run(f, task != "L6_motor_plate_iter")  # motor plate states are full rebuilds
        st.append({"prompt": prompt, "bodies": bodies()})
        print(task, f, [(b["name"], b["volume"]) for b in st[-1]["bodies"]])
    out[task] = {"setup": rd(start), "steps": st}
json.dump(out, open(os.path.join(BENCH, "specsL6.json"), "w", encoding="utf-8", newline="\n"), indent=1)
