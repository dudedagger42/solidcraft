"""Writes specsRel.json: parts described the way a person would (centred, evenly spaced, stacked) instead of by coordinates."""
import json, math, os
pi = math.pi
S = {}


def part(task, spec, bodies):
    S[task] = {"spec": spec, "bodies": [{"name": n, "volume": round(v, 4), "bbox": [[round(c, 4) for c in p] for p in bb]} for n, v, bb in bodies]}


part("bushplate", "A square bushing plate as one solid: 90 mm square and 10 mm thick, centred on the Z axis and lying on the XY plane (bottom face at z=0). A 30 mm bore through its middle. Eight 6 mm holes through it, evenly spaced on a 66 mm bolt circle around the bore, with one of them on the +X axis.",
     [("bushplate", 90 * 90 * 10 - pi * 225 * 10 - 8 * pi * 9 * 10, [[-45, -45, 0], [45, 45, 10]])])

part("slotcube", "A slotted block as one solid: a 50 mm cube sitting on the XY plane with one corner at the origin and the rest in positive X, Y and Z. A slot 30 mm wide and 20 mm deep is cut into the top face, centred on the top face and running the full length of the cube along Y.",
     [("slotcube", 50 ** 3 - 30 * 20 * 50, [[0, 0, 0], [50, 50, 50]])])

R, r = 15, 12
part("tee", "A pipe tee as one solid. Both pipes are tubes with 30 mm outside diameter and 24 mm inside diameter. The main pipe lies along the X axis, centred on the origin, 100 mm long. The branch pipe leaves the main pipe's side along +Y: its axis is the Y axis and it ends 50 mm from the main pipe's axis. The bores are open into each other (the branch bore runs from the main pipe's axis out to the branch end), and both main pipe ends and the branch end are open.",
     [("tee", (pi * R * R * 100 + pi * R * R * 50 - 8 / 3 * R ** 3) - (pi * r * r * 100 + pi * r * r * 50 - 8 / 3 * r ** 3), [[-50, -15, -15], [50, 50, 15]])])

part("washers", "A pin with a stack of washers: an assembly of 6 separate parts (each its own part/component, or its own separate solid if your tools have no parts). Pin: a 10 mm diameter, 30 mm long cylinder standing upright on the origin (bottom at z=0, on the Z axis). Five identical washers named W1..W5 (outside diameter 20 mm, inside diameter 10 mm, 2 mm thick) are threaded onto the pin and stacked from the bottom: W1 rests on the XY plane, each next one sits directly on the one below. The parts stay separate solids; do not merge them.",
     [("Pin", pi * 25 * 30, [[-5, -5, 0], [5, 5, 30]])] +
     [(f"W{k + 1}", pi * (100 - 25) * 2, [[-10, -10, 2 * k], [10, 10, 2 * k + 2]]) for k in range(5)])

json.dump(S, open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "specsRel.json"), "w", encoding="utf-8"), indent=1)
print({k: v["bodies"][0]["volume"] for k, v in S.items()})
