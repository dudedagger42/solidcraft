"""Writes specsEdit.json: change an existing 24-part assembly (rack + board on standoffs).

Each spec has "setup" (hybrid script that builds the starting design) and the expected bodies after the change.
"""
import json, math, os
pi = math.pi
S = {}

SETUP = """for i,(x,y) in enumerate([(0,0),(280,0),(0,180),(280,180)]):
    with part(f'P{i+1}'): box(20,20,400,at=(x,y,0),name=f'P{i+1}')
for i,z in enumerate((50,150,250,350)):
    with part(f'Shelf{i+1}'):
        box(300,200,10,at=(0,0,z),name=f'Shelf{i+1}')
        for x,y in [(0,0),(280,0),(0,180),(280,180)]: box(20,20,10,at=(x,y,z),op='cut')
with part('Board'):
    box(100,80,1.6,at=(100,60,70),name='Board')
    for y in (70,100,130):
        for x in (110,130,150,170,190): hole(at=(x,y,71.6),d=3.2)
i=0
for y in (70,100,130):
    for x in (110,130,150,170,190):
        i+=1
        with part(f'S{i}'):
            cylinder(10,d=6,at=(x,y,60),name=f'S{i}')
            hole(at=(x,y,70),d=3)"""

INTRO = ("The design open in SolidCraft is a shelving rack: four 20 x 20 mm posts P1..P4 at the corners of a 300 x 200 mm footprint, and four 10 mm shelves Shelf1..Shelf4 "
         "(with corner notches for the posts) at z = 50, 150, 250, 350. A circuit board named Board sits on fifteen standoffs S1..S15 that stand on Shelf1. "
         "Each is its own part. ")
POSTS = [(0, 0), (280, 0), (0, 180), (280, 180)]
GRID = [(x, y) for y in (70, 100, 130) for x in (110, 130, 150, 170, 190)]
SHELF = 300 * 200 * 10 - 4 * 400 * 10
BOARD = 100 * 80 * 1.6 - 15 * pi * 1.6 ** 2 * 1.6
STAND = pi * (9 - 2.25) * 10


def bodies(post_h=400, shelves=(50, 150, 250, 350), shelf_vol=None, stand_z=60, drop=()):
    out = [(f"P{i + 1}", 400 * post_h, [[x, y, 0], [x + 20, y + 20, post_h]]) for i, (x, y) in enumerate(POSTS)]
    out += [(f"Shelf{i + 1}", shelf_vol or SHELF, [[0, 0, z], [300, 200, z + 10]]) for i, z in enumerate(shelves)]
    out.append(("Board", BOARD, [[100, 60, stand_z + 10], [200, 140, stand_z + 11.6]]))
    out += [(f"S{i + 1}", STAND, [[x - 3, y - 3, stand_z], [x + 3, y + 3, stand_z + 10]]) for i, (x, y) in enumerate(GRID) if f"S{i + 1}" not in drop]
    return out


def part(task, spec, b):
    S[task] = {"spec": INTRO + spec, "setup": SETUP, "edit": True,
              "bodies": [{"name": n, "volume": round(v, 4), "bbox": [[round(c, 4) for c in p] for p in bb]} for n, v, bb in b]}


part("edit_move", "Change it: move the board together with all fifteen standoffs up so the standoffs stand on Shelf2 instead of Shelf1 (same x and y positions), and raise Shelf4 by 20 mm. Change nothing else.",
     bodies(shelves=(50, 150, 250, 370), stand_z=160))
part("edit_add", "Change it: make the four posts 450 mm tall instead of 400 (still starting at z=0), and add a fifth shelf named Shelf5, the same as the other shelves (including the corner notches), with its bottom face at z=430. Change nothing else.",
     bodies(post_h=450, shelves=(50, 150, 250, 350, 430)))
part("edit_holes", "Change it: drill a 6 mm diameter hole through each of the four shelves along Z, centred at x=150, y=170. Also delete the standoffs S1 and S15 (the board keeps its holes). Change nothing else.",
     bodies(shelf_vol=SHELF - pi * 9 * 10, drop=("S1", "S15")))

json.dump(S, open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "specsEdit.json"), "w", encoding="utf-8"), indent=1)
print({k: len(v["bodies"]) for k, v in S.items()})
