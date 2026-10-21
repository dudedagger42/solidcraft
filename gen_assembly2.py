"""A synthetic STEP (AP214) assembly made only of boxes, public-safe by construction.

From an anonymous tree (step_shape.py --tree): the same products, nesting, reuse and solids per
part, every part a box (or several), every name P<n>.

    python gen_assembly.py --tree tree.json --out synthetic.step [--repeat K]

Or from numbers alone, a balanced tree:

    python gen_assembly.py --fanout 8 --depth 3 --unique 0.3 --solids 1 --out synthetic.step

--repeat K places the roots' children K times over (more occurrences, same parts), to scale a
case up until it shows the problem.
"""
import argparse
import json
import random

ap = argparse.ArgumentParser()
ap.add_argument("--tree")
ap.add_argument("--fanout", type=int, default=8)
ap.add_argument("--depth", type=int, default=3)
ap.add_argument("--unique", type=float, default=0.3, help="share of leaf parts that are their own product")
ap.add_argument("--solids", type=int, default=1)
ap.add_argument("--repeat", type=int, default=1)
ap.add_argument("--seed", type=int, default=1)
ap.add_argument("--hole", type=float, default=0.3, help="hole radius as a share of the hole spacing")
ap.add_argument("--max-holes", type=int, default=400, help="at most this many holes in one solid")
ap.add_argument("--hole-share", type=float, default=1.0, help="holes per curved face (to match a triangle count)")
ap.add_argument("--out", required=True)
a = ap.parse_args()
rng = random.Random(a.seed)

if a.tree:
    tree = json.load(open(a.tree))
else:
    # Balanced: depth levels of sub-assemblies, `fanout` children each; leaves drawn from a pool
    # so only `unique` of them are distinct products.
    counter = [0]

    def fresh():
        counter[0] += 1
        return f"P{counter[0]}"

    pool = []

    def leaf():
        if pool and rng.random() > a.unique:
            return {"id": rng.choice(pool), "solids": a.solids}
        pid = fresh()
        pool.append(pid)
        return {"id": pid, "solids": a.solids}

    def build(level):
        if level == a.depth:
            return leaf()
        return {"id": fresh(), "children": [build(level + 1) for _ in range(a.fanout)]}

    tree = {"roots": [build(0)]}

# Products by id, children as ids (a "ref" node points back at an id already given).
products = {}


def collect(node):
    pid = node.get("id") or node.get("ref")
    if "children" in node:
        kids = [collect(c) for c in node["children"]]
        products.setdefault(pid, {"children": kids})
    elif "id" in node:
        products.setdefault(pid, {"solids": node.get("solids", 1), "curved": node.get("curved", 0)})
    return pid


roots = [collect(r) for r in tree["roots"]]
if a.repeat > 1:
    for r in roots:
        if "children" in products[r]:
            products[r]["children"] = products[r]["children"] * a.repeat

lines = []
n = [0]


def e(body):
    n[0] += 1
    lines.append(f"#{n[0]}={body};")
    return f"#{n[0]}"


def pt(x, y, z):
    return e(f"CARTESIAN_POINT('',({x:.4f},{y:.4f},{z:.4f}))")


def d(x, y, z):
    return e(f"DIRECTION('',({x:.1f},{y:.1f},{z:.1f}))")


def axis(o=(0, 0, 0)):
    return e(f"AXIS2_PLACEMENT_3D('',{pt(*o)},{d(0, 0, 1)},{d(1, 0, 0)})")


app = e("APPLICATION_CONTEXT('automotive design')")
e(f"APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',2000,{app})")
pctx = e(f"PRODUCT_CONTEXT('',{app},'mechanical')")
pdctx = e(f"PRODUCT_DEFINITION_CONTEXT('part definition',{app},'design')")
mm = e("(LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.))")
rad = e("(NAMED_UNIT(*) PLANE_ANGLE_UNIT() SI_UNIT($,.RADIAN.))")
sr = e("(NAMED_UNIT(*) SI_UNIT($,.STERADIAN.) SOLID_ANGLE_UNIT())")
unc = e(f"UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-07),{mm},'distance_accuracy_value','')")
ctx = e(f"(GEOMETRIC_REPRESENTATION_CONTEXT(3) GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT(({unc})) GLOBAL_UNIT_ASSIGNED_CONTEXT(({mm},{rad},{sr})) REPRESENTATION_CONTEXT('',''))")

# Box faces as corner bits (x, y, z), counter-clockwise seen from outside, with the outward normal.
FACES = [
    ([(0, 0, 0), (0, 1, 0), (1, 1, 0), (1, 0, 0)], (0, 0, -1)),
    ([(0, 0, 1), (1, 0, 1), (1, 1, 1), (0, 1, 1)], (0, 0, 1)),
    ([(0, 0, 0), (1, 0, 0), (1, 0, 1), (0, 0, 1)], (0, -1, 0)),
    ([(0, 1, 0), (0, 1, 1), (1, 1, 1), (1, 1, 0)], (0, 1, 0)),
    ([(0, 0, 0), (0, 0, 1), (0, 1, 1), (0, 1, 0)], (-1, 0, 0)),
    ([(1, 0, 0), (1, 1, 0), (1, 1, 1), (1, 0, 1)], (1, 0, 0)),
]


def box(lo, hi):
    corner = lambda b: tuple(hi[i] if b[i] else lo[i] for i in range(3))
    verts = {}
    for bx in (0, 1):
        for by in (0, 1):
            for bz in (0, 1):
                verts[(bx, by, bz)] = e(f"VERTEX_POINT('',{pt(*corner((bx, by, bz)))})")
    edges = {}
    for b in verts:
        for ax in range(3):
            if b[ax] == 0:
                c = list(b)
                c[ax] = 1
                c = tuple(c)
                dv = [0, 0, 0]
                dv[ax] = 1
                line = e(f"LINE('',{pt(*corner(b))},{e(f'VECTOR({chr(39)}{chr(39)},{d(*dv)},1.)')})")
                edges[(b, c)] = e(f"EDGE_CURVE('',{verts[b]},{verts[c]},{line},.T.)")
    faces = []
    for corners, normal in FACES:
        oes = []
        for i in range(4):
            p, q = corners[i], corners[(i + 1) % 4]
            if (p, q) in edges:
                oes.append(e(f"ORIENTED_EDGE('',*,*,{edges[(p, q)]},.T.)"))
            else:
                oes.append(e(f"ORIENTED_EDGE('',*,*,{edges[(q, p)]},.F.)"))
        loop = e(f"EDGE_LOOP('',({','.join(oes)}))")
        bound = e(f"FACE_OUTER_BOUND('',{loop},.T.)")
        ref = (1, 0, 0) if normal[0] == 0 else (0, 1, 0)
        plane = e(f"PLANE('',{e(f'AXIS2_PLACEMENT_3D({chr(39)}{chr(39)},{pt(*corner(corners[0]))},{d(*normal)},{d(*ref)})')})")
        faces.append(e(f"ADVANCED_FACE('',({bound}),{plane},.T.)"))
    shell = e(f"CLOSED_SHELL('',({','.join(faces)}))")
    return e(f"MANIFOLD_SOLID_BREP('',{shell})")


def drilled(lo, hi, holes):
    """A box with `holes` round through-holes along z on a grid: one cylindrical face each, the
    top and bottom faces bounded inside by the holes' circles (counts like a real part's curved
    faces, nothing else of it)."""
    if holes <= 0:
        return box(lo, hi)
    cols = max(1, round(holes ** 0.5))
    rows = -(-holes // cols)
    sx, sy = (hi[0] - lo[0]) / cols, (hi[1] - lo[1]) / rows
    r = min(sx, sy) * a.hole
    z0, z1 = lo[2], hi[2]
    corner = lambda b: tuple(hi[i] if b[i] else lo[i] for i in range(3))
    verts = {}
    for bx in (0, 1):
        for by in (0, 1):
            for bz in (0, 1):
                verts[(bx, by, bz)] = e(f"VERTEX_POINT('',{pt(*corner((bx, by, bz)))})")
    edges = {}
    for b in verts:
        for ax in range(3):
            if b[ax] == 0:
                c = list(b)
                c[ax] = 1
                c = tuple(c)
                dv = [0, 0, 0]
                dv[ax] = 1
                line = e(f"LINE('',{pt(*corner(b))},{e(f'VECTOR({chr(39)}{chr(39)},{d(*dv)},1.)')})")
                edges[(b, c)] = e(f"EDGE_CURVE('',{verts[b]},{verts[c]},{line},.T.)")
    circles = []  # (bottom edge, top edge, centre)
    for k in range(holes):
        cx, cy = lo[0] + (k % cols + 0.5) * sx, lo[1] + (k // cols + 0.5) * sy
        ends = []
        for z in (z0, z1):
            v = e(f"VERTEX_POINT('',{pt(cx + r, cy, z)})")
            c = e(f"CIRCLE('',{e(f'AXIS2_PLACEMENT_3D({chr(39)}{chr(39)},{pt(cx, cy, z)},{d(0, 0, 1)},{d(1, 0, 0)})')},{r:.4f})")
            ends.append(e(f"EDGE_CURVE('',{v},{v},{c},.T.)"))
        circles.append((ends[0], ends[1], (cx, cy)))
    faces = []
    for fi, (corners, normal) in enumerate(FACES):
        oes = []
        for i in range(4):
            p, q = corners[i], corners[(i + 1) % 4]
            if (p, q) in edges:
                oes.append(e(f"ORIENTED_EDGE('',*,*,{edges[(p, q)]},.T.)"))
            else:
                oes.append(e(f"ORIENTED_EDGE('',*,*,{edges[(q, p)]},.F.)"))
        loop = e(f"EDGE_LOOP('',({','.join(oes)}))")
        bounds = [e(f"FACE_OUTER_BOUND('',{loop},.T.)")]
        # Bottom (normal -z, index 0) and top (normal +z, index 1) take the holes as inner loops,
        # turning the other way round from their outer loop.
        if fi in (0, 1):
            for bot, top, _ in circles:
                edge, sense = (bot, ".T.") if fi == 0 else (top, ".F.")
                bounds.append(e(f"FACE_BOUND('',{e(f'EDGE_LOOP({chr(39)}{chr(39)},({e(f'ORIENTED_EDGE({chr(39)}{chr(39)},*,*,{edge},{sense})')}))')},.T.)"))
        ref = (1, 0, 0) if normal[0] == 0 else (0, 1, 0)
        plane = e(f"PLANE('',{e(f'AXIS2_PLACEMENT_3D({chr(39)}{chr(39)},{pt(*corner(corners[0]))},{d(*normal)},{d(*ref)})')})")
        faces.append(e(f"ADVANCED_FACE('',({','.join(bounds)}),{plane},.T.)"))
    for bot, top, (cx, cy) in circles:
        surf = e(f"CYLINDRICAL_SURFACE('',{e(f'AXIS2_PLACEMENT_3D({chr(39)}{chr(39)},{pt(cx, cy, z0)},{d(0, 0, 1)},{d(1, 0, 0)})')},{r:.4f})")
        lb = e(f"FACE_BOUND('',{e(f'EDGE_LOOP({chr(39)}{chr(39)},({e(f'ORIENTED_EDGE({chr(39)}{chr(39)},*,*,{bot},.F.)')}))')},.T.)")
        lt = e(f"FACE_BOUND('',{e(f'EDGE_LOOP({chr(39)}{chr(39)},({e(f'ORIENTED_EDGE({chr(39)}{chr(39)},*,*,{top},.T.)')}))')},.T.)")
        # The hole's wall faces its axis: against the cylinder's own outward normal.
        faces.append(e(f"ADVANCED_FACE('',({lb},{lt}),{surf},.F.)"))
    shell = e(f"CLOSED_SHELL('',({','.join(faces)}))")
    return e(f"MANIFOLD_SOLID_BREP('',{shell})")


made = {}  # id -> (product definition, representation, its origin axis)
order = []


def visit(pid, stack=()):
    if pid in made or pid in stack:
        return
    for c in products[pid].get("children", []):
        visit(c, stack + (pid,))
    order.append(pid)
    made[pid] = None


for r in roots:
    visit(r)

for k, pid in enumerate(order):
    p = products[pid]
    prod = e(f"PRODUCT('{pid}','{pid}','',({pctx}))")
    pdf = e(f"PRODUCT_DEFINITION_FORMATION('','',{prod})")
    pd = e(f"PRODUCT_DEFINITION('design','',{pdf},{pdctx})")
    e(f"PRODUCT_RELATED_PRODUCT_CATEGORY('part',$,({prod}))")
    pds = e(f"PRODUCT_DEFINITION_SHAPE('','',{pd})")
    origin = axis()
    if "children" in p:
        placements = []
        kids = p["children"]
        cols = max(1, round(len(kids) ** 0.5))
        for i, c in enumerate(kids):
            cpd, crep, corigin = made[c]
            at = axis(((i % cols) * 40.0, (i // cols) * 40.0, 0.0))
            placements.append((c, cpd, crep, corigin, at))
        rep = e(f"SHAPE_REPRESENTATION('',({','.join([origin] + [x[4] for x in placements])}),{ctx})")
        e(f"SHAPE_DEFINITION_REPRESENTATION({pds},{rep})")
        for i, (c, cpd, crep, corigin, at) in enumerate(placements):
            nauo = e(f"NEXT_ASSEMBLY_USAGE_OCCURRENCE('{pid}.{i}','{c}:{i + 1}','',{pd},{cpd},$)")
            npds = e(f"PRODUCT_DEFINITION_SHAPE('','',{nauo})")
            t = e(f"ITEM_DEFINED_TRANSFORMATION('','',{corigin},{at})")
            rr = e(f"(REPRESENTATION_RELATIONSHIP('','',{crep},{rep}) REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION({t}) SHAPE_REPRESENTATION_RELATIONSHIP())")
            e(f"CONTEXT_DEPENDENT_SHAPE_REPRESENTATION({rr},{npds})")
    else:
        n_solids = max(1, p.get("solids", 1))
        holes = min(a.max_holes, round(a.hole_share * p.get("curved", 0) / n_solids))
        size = (5.0 + (k % 7) * 3.0) * max(1.0, (holes / 4.0) ** 0.5)
        solids = [drilled((s * (size + 2.0), 0.0, 0.0), (s * (size + 2.0) + size, size * 0.8, size * 0.6), holes) for s in range(p.get("solids", 1))]
        rep = e(f"ADVANCED_BREP_SHAPE_REPRESENTATION('',({','.join([origin] + solids)}),{ctx})")
        e(f"SHAPE_DEFINITION_REPRESENTATION({pds},{rep})")
    made[pid] = (pd, rep, origin)

with open(a.out, "w", newline="\n") as f:
    f.write("ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('synthetic assembly'),'2;1');\n")
    f.write("FILE_NAME('synthetic.step','2026-01-01T00:00:00',(''),(''),'gen_assembly.py','','');\n")
    f.write("FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }'));\nENDSEC;\nDATA;\n")
    f.write("\n".join(lines))
    f.write("\nENDSEC;\nEND-ISO-10303-21;\n")

leaves = sum(1 for p in products.values() if "children" not in p)


def expanded(pid):
    p = products[pid]
    return 1 + sum(expanded(c) for c in p.get("children", []))


print(f"{a.out}: {len(products)} products ({leaves} parts), {sum(expanded(r) for r in roots) - len(roots)} occurrences, {n[0]} entities")
