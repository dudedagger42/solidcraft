"""Writes specs12.json: 10 single parts and 2 assemblies, each with exact expected bodies (volume, bbox)."""
import json, math, os
pi = math.pi
S = {}


def part(task, spec, bodies):
    S[task] = {"spec": spec, "bodies": [{"name": n, "volume": round(v, 4), "bbox": bb} for n, v, bb in bodies]}


part("plate", "A rectangular plate 60 mm (X) by 40 mm (Y) by 8 mm thick (Z), spanning x 0..60, y 0..40, z 0..8. Four through holes of diameter 6 mm go straight down through it (along Z), centred at (8,8), (52,8), (8,32) and (52,32).",
     [("plate", 19200 - 4 * pi * 9 * 8, [[0, 0, 0], [60, 40, 8]])])

part("bracket", "An L-bracket as one solid. Base: 80 (X) by 40 (Y) by 6 (Z) mm, spanning x 0..80, y 0..40, z 0..6. Upright wall: 80 (X) by 6 (Y) mm, standing on the back edge of the base, spanning x 0..80, y 34..40, z 0..50. The inside corner where the top of the base meets the front face of the wall (the edge along X at y=34, z=6) has a 5 mm radius fillet. The base has two through slots (along Z), each 6 mm wide with fully rounded ends and 20 mm long overall in X; the slot centres are at (20, 17) and (60, 17).",
     [("bracket", 80 * 40 * 6 + 80 * 6 * 44 + 25 * (1 - pi / 4) * 80 - 2 * 6 * (14 * 6 + pi * 9), [[0, 0, 0], [80, 40, 50]])])

part("enclosure", "An open-top electronics enclosure as one solid. Outer block 100 (X) by 60 (Y) by 40 (Z) mm, spanning x 0..100, y 0..60, z 0..40. Round its four vertical outside edges with a 5 mm radius. Hollow it out with a 2.5 mm wall on the four sides and a 2.5 mm floor, open at the top (z=40), so the cavity follows the outside shape 2.5 mm in. Inside, four cylindrical screw bosses of diameter 8 mm stand on the floor (from z=2.5 up to z=40), centred at (8,8), (92,8), (8,52) and (92,52); each has a 3 mm diameter hole 30 mm deep drilled down from its top (z=40 to z=10). Finally a 10 mm diameter hole goes through the front wall only (the wall at y=0..2.5), centred at x=50, z=20, along Y.",
     [("enclosure", (240000 - 4 * (1 - pi / 4) * 25 * 40) - ((95 * 55 - 4 * (1 - pi / 4) * 6.25) * 37.5) + 4 * pi * 16 * 37.5 - 4 * pi * 2.25 * 30 - pi * 25 * 2.5,
       [[0, 0, 0], [100, 60, 40]])])

part("flange", "A round flange as one solid, on the Z axis. A disc of diameter 80 mm and 10 mm thick (z 0..10), centred on the Z axis. On top a hub of diameter 40 mm from z=10 to z=30. A bore of diameter 20 mm goes through the whole flange along Z. Six bolt holes of diameter 8 mm go through the disc along Z, equally spaced on a 60 mm diameter circle, the first one centred at (30, 0).",
     [("flange", pi * 1600 * 10 + pi * 400 * 20 - pi * 100 * 30 - 6 * pi * 16 * 10, [[-40, -40, 0], [40, 40, 30]])])

part("shaft", "A stepped round shaft as one solid along the X axis (its axis is the X axis). From x=0 to 30 it is 20 mm in diameter, from x=30 to 70 it is 30 mm, and from x=70 to 95 it is 16 mm. The two outer end edges (the circular edge at x=0 and the circular edge at x=95) have a 1 mm x 45 degree chamfer; no other edges are chamfered.",
     [("shaft", pi * 100 * 30 + pi * 225 * 40 + pi * 64 * 25 - pi * (10 - 1 / 3) - pi * (8 - 1 / 3), [[0, -15, -15], [95, 15, 15]])])

r_v = 12 / math.cos(math.radians(30))
part("spacer", "A hexagonal spacer as one solid: a regular hexagonal prism 24 mm across the flats and 10 mm tall (z 0..10), centred on the Z axis, with two of its flats parallel to the X axis (flats at y = -12 and y = +12). A 12 mm diameter hole goes through it along the Z axis.",
     [("spacer", 2 * math.sqrt(3) * 144 * 10 - pi * 36 * 10, [[-r_v, -12, 0], [r_v, 12, 10]])])

part("channel", "A U-channel as one solid, 50 mm long along X (x 0..50). Its cross-section (in Y and Z) is 40 mm wide (y 0..40) and 30 mm tall (z 0..30): a 4 mm thick bottom (z 0..4) and two 4 mm thick side walls (y 0..4 and y 36..40), open at the top. Two 6 mm diameter holes go through the bottom along Z, centred at (15, 20) and (35, 20).",
     [("channel", (1200 - 32 * 26) * 50 - 2 * pi * 9 * 4, [[0, 0, 0], [50, 40, 30]])])

a, b, c, r = 44, 24, 14, 3
part("rounded", "A rounded block as one solid: a box 50 (X) by 30 (Y) by 20 (Z) mm spanning x 0..50, y 0..30, z 0..20, with all twelve of its edges rounded with a 3 mm radius fillet.",
     [("rounded", a * b * c + 2 * r * (a * b + b * c + a * c) + pi * r * r * (a + b + c) + 4 / 3 * pi * r ** 3, [[0, 0, 0], [50, 30, 20]])])

part("frustum", "A cone frustum as one solid on the Z axis: base diameter 40 mm at z=0, top diameter 20 mm at z=30. A 10 mm diameter hole goes through it along the Z axis.",
     [("frustum", pi * 30 / 3 * (400 + 200 + 100) - pi * 25 * 30, [[-20, -20, 0], [20, 20, 30]])])

part("rail", "A mounting rail as one solid: a plate 100 (X) by 20 (Y) by 5 (Z) mm spanning x 0..100, y 0..20, z 0..5, with eight 4 mm diameter through holes along Z in a row at y=10, centred at x = 15, 25, 35, 45, 55, 65, 75 and 85.",
     [("rail", 10000 - 8 * pi * 4 * 5, [[0, 0, 0], [100, 20, 5]])])

part("clamp", "A pinned clamp assembly made of four separate parts (each its own part/component, or its own separate solid if your tools have no parts), named Base, Lid, Pin1 and Pin2. Base: a block spanning x 0..60, y 0..40, z 0..20, with two blind holes of diameter 5 mm drilled down from its top face 15 mm deep (z 20 down to 5), centred at (10, 20) and (50, 20). Lid: a plate spanning x 0..60, y 0..40, z 20..25 sitting on the Base, with two 5 mm diameter through holes along Z at the same two centres. Pin1 and Pin2: cylinders of diameter 5 mm from z=5 to z=25, centred at (10, 20) and (50, 20). The parts stay separate solids; do not merge them.",
     [("Base", 48000 - 2 * pi * 6.25 * 15, [[0, 0, 0], [60, 40, 20]]),
      ("Lid", 12000 - 2 * pi * 6.25 * 5, [[0, 0, 20], [60, 40, 25]]),
      ("Pin1", pi * 6.25 * 20, [[7.5, 17.5, 5], [12.5, 22.5, 25]]),
      ("Pin2", pi * 6.25 * 20, [[47.5, 17.5, 5], [52.5, 22.5, 25]])])

part("bearing", "A shaft-and-housing assembly made of four separate parts (each its own part/component, or its own separate solid if your tools have no parts), named Housing, Shaft, CollarL and CollarR. Housing: a block spanning x 0..80, y 0..50, z 0..40, with a 30 mm diameter bore going through it along X whose axis is the line y=25, z=20; and four 6 mm diameter mounting holes going through it along Z, centred at (10, 6), (70, 6), (10, 44) and (70, 44). Shaft: a 30 mm diameter cylinder on the same axis (y=25, z=20), from x=-20 to x=100. CollarL and CollarR: tubes with 40 mm outside diameter and 30 mm inside diameter, each 10 mm long, on the same axis, CollarL from x=-12 to x=-2 and CollarR from x=82 to x=92. The parts stay separate solids; do not merge them.",
     [("Housing", 160000 - pi * 225 * 80 - 4 * pi * 9 * 40, [[0, 0, 0], [80, 50, 40]]),
      ("Shaft", pi * 225 * 120, [[-20, 10, 5], [100, 40, 35]]),
      ("CollarL", pi * (400 - 225) * 10, [[-12, 5, 0], [-2, 45, 40]]),
      ("CollarR", pi * (400 - 225) * 10, [[82, 5, 0], [92, 45, 40]])])

json.dump(S, open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "specs12.json"), "w"), indent=1)
for k, v in S.items():
    print(k, [(b["name"], round(b["volume"], 2)) for b in v["bodies"]])
