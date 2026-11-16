"""Writes specsBig.json: larger parts and assemblies (many features / many parts) with exact expected bodies."""
import json, math, os
pi = math.pi
S = {}


def part(task, spec, bodies):
    S[task] = {"spec": spec, "bodies": [{"name": n, "volume": round(v, 4), "bbox": [[round(c, 4) for c in p] for p in bb]} for n, v, bb in bodies]}


part("heatsink", "A finned heatsink as one solid. Base plate: x 0..80, y 0..60, z 0..5. Sixteen identical fins stand on the base, each 2 mm thick in X, running the full depth y 0..60, from z=5 up to z=30. Fin number i (i = 0..15) spans x = 1.5+5i .. 3.5+5i (so the first fin is x 1.5..3.5 and the last is x 76.5..78.5).",
     [("heatsink", 80 * 60 * 5 + 16 * 2 * 60 * 25, [[0, 0, 0], [80, 60, 30]])])

grid = [(x, y) for y in (10, 40, 70) for x in (10, 30, 50, 70, 90)]
part("standoffs", "A circuit board on standoffs: an assembly of 16 separate parts (each its own part/component, or its own separate solid if your tools have no parts). Board: x 0..100, y 0..80, z 10..11.6, with fifteen 3.2 mm diameter through holes along Z, one at every combination of x = 10, 30, 50, 70, 90 and y = 10, 40, 70. Under each hole stands a standoff (15 standoffs, named S1..S15): a cylinder of diameter 6 mm from z=0 to z=10 on the same centre, with a 3 mm diameter hole through it along Z. The parts stay separate solids; do not merge them.",
     [("Board", 100 * 80 * 1.6 - 15 * pi * 1.6 ** 2 * 1.6, [[0, 0, 10], [100, 80, 11.6]])] +
     [(f"S{i + 1}", pi * (9 - 2.25) * 10, [[x - 3, y - 3, 0], [x + 3, y + 3, 10]]) for i, (x, y) in enumerate(grid)])

posts = [(0, 0), (280, 0), (0, 180), (280, 180)]
part("rack", "A shelving rack: an assembly of 8 separate parts (each its own part/component, or its own separate solid if your tools have no parts). Four square posts named P1..P4, each 20 x 20 mm in section and 400 mm tall (z 0..400), occupying x 0..20 or 280..300 and y 0..20 or 180..200 (one post at each of the four corners). Four shelves named Shelf1..Shelf4, each a 10 mm thick board covering x 0..300, y 0..200, with their bottom faces at z = 50, 150, 250 and 350; each shelf has a 20 x 20 mm square notch cut out at each of its four corners where the posts pass, so shelves and posts do not overlap. The parts stay separate solids; do not merge them.",
     [(f"P{i + 1}", 20 * 20 * 400, [[x, y, 0], [x + 20, y + 20, 400]]) for i, (x, y) in enumerate(posts)] +
     [(f"Shelf{i + 1}", 300 * 200 * 10 - 4 * 400 * 10, [[0, 0, z], [300, 200, z + 10]]) for i, z in enumerate((50, 150, 250, 350))])

fl = pi * (3600 - 625) * 12 - 8 * pi * 25 * 12
bolts = [(47.5 * math.cos(math.radians(45 * k)), 47.5 * math.sin(math.radians(45 * k))) for k in range(8)]
part("flangepair", "A bolted flange joint on the Z axis: an assembly of 11 separate parts (each its own part/component, or its own separate solid if your tools have no parts). FlangeA: a disc of diameter 120 mm from z=0 to z=12, centred on the Z axis, with a 50 mm diameter bore through it along Z and eight 10 mm diameter bolt holes through it along Z, equally spaced on a 95 mm diameter circle with the first centred at (47.5, 0). Gasket: a ring of outside diameter 80 mm and inside diameter 50 mm from z=12 to z=14. FlangeB: identical to FlangeA but from z=14 to z=26 (holes in the same places). Eight bolts named B1..B8: plain cylinders of diameter 10 mm from z=-5 to z=31, one in each bolt hole. The parts stay separate solids; do not merge them.",
     [("FlangeA", fl, [[-60, -60, 0], [60, 60, 12]]), ("Gasket", pi * (1600 - 625) * 2, [[-40, -40, 12], [40, 40, 14]]),
      ("FlangeB", fl, [[-60, -60, 14], [60, 60, 26]])] +
     [(f"B{k + 1}", pi * 25 * 36, [[x - 5, y - 5, -5], [x + 5, y + 5, 31]]) for k, (x, y) in enumerate(bolts)])

json.dump(S, open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "specsBig.json"), "w", encoding="utf-8"), indent=1)
print({k: len(v["bodies"]) for k, v in S.items()})
