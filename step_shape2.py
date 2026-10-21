"""The SHAPE of a STEP assembly, with nothing from its contents: no names, no ids from the file,
no dimensions, no metadata. Prints counts only, and writes an anonymous tree (products renamed
P0, P1, ... in order of first use) that gen_assembly.py rebuilds with plain boxes.

    python step_shape.py private.step [--tree tree.json]
"""
import json
import re
import sys
from collections import Counter, defaultdict

path = sys.argv[1]
tree_out = sys.argv[sys.argv.index("--tree") + 1] if "--tree" in sys.argv else None

text = open(path, encoding="latin-1").read()
data = text[text.find("DATA;") :]
# Entities: #id = TYPE(args); (complex entities "( A() B() )" are kept as one line).
ent = {}
kind = {}
for m in re.finditer(r"#(\d+)\s*=\s*(.*?);\s*(?=#\d+\s*=|ENDSEC)", data, re.S):
    body = m.group(2).strip()
    k = re.match(r"([A-Z_0-9]+)", body)
    ent[int(m.group(1))] = body
    kind[int(m.group(1))] = k.group(1) if k else "COMPLEX"
refs = lambda body: [int(r) for r in re.findall(r"#(\d+)", body)]

counts = Counter(kind.values())
pds = [i for i, k in kind.items() if k == "PRODUCT_DEFINITION"]
nauo = [i for i, k in kind.items() if k == "NEXT_ASSEMBLY_USAGE_OCCURRENCE"]
# NAUO('id','name','desc', #parent_pd, #child_pd, ref_designator)
children = defaultdict(list)
parents = defaultdict(list)
for i in nauo:
    r = refs(ent[i])
    if len(r) >= 2:
        children[r[0]].append(r[1])
        parents[r[1]].append(r[0])

# Solids per product definition: PDS -> SDR -> shape representation -> its items, following
# shape representation relationships (the usual AP214/AP242 layout) down to the B-reps.
solid_kinds = {"MANIFOLD_SOLID_BREP", "BREP_WITH_VOIDS", "FACETED_BREP"}
pds_of = {}  # PRODUCT_DEFINITION_SHAPE -> PD
for i, k in kind.items():
    if k == "PRODUCT_DEFINITION_SHAPE":
        r = refs(ent[i])
        if r:
            pds_of[i] = r[-1]
rep_of_pd = defaultdict(list)
for i, k in kind.items():
    if k == "SHAPE_DEFINITION_REPRESENTATION":
        r = refs(ent[i])
        if len(r) >= 2 and r[0] in pds_of:
            rep_of_pd[pds_of[r[0]]].append(r[1])
rel = defaultdict(list)
for i, k in kind.items():
    if k in ("SHAPE_REPRESENTATION_RELATIONSHIP",) or (k == "COMPLEX" and "SHAPE_REPRESENTATION_RELATIONSHIP" in ent[i] and "TRANSFORMATION" not in ent[i]):
        r = refs(ent[i])
        if len(r) >= 2:
            rel[r[0]].append(r[1])
            rel[r[1]].append(r[0])


def solids_under(rep, seen):
    if rep in seen:
        return 0
    seen.add(rep)
    n = 0
    for r in refs(ent.get(rep, "")):
        if kind.get(r) in solid_kinds:
            n += 1
    for other in rel.get(rep, []):
        if kind.get(other, "").endswith("REPRESENTATION") or "REPRESENTATION" in kind.get(other, ""):
            n += solids_under(other, seen)
    return n


solids = {pd: sum(solids_under(rep, set()) for rep in rep_of_pd.get(pd, [])) for pd in pds}

# Faces of a product's solids, and how many of them are curved: counts only.
curved_kinds = ("CYLINDRICAL_SURFACE", "CONICAL_SURFACE", "TOROIDAL_SURFACE", "SPHERICAL_SURFACE", "B_SPLINE_SURFACE", "SURFACE_OF_REVOLUTION", "SURFACE_OF_LINEAR_EXTRUSION")


def faces_of_solid(sid):
    faces, curved = 0, 0
    for shell in refs(ent.get(sid, "")):
        if kind.get(shell) not in ("CLOSED_SHELL", "OPEN_SHELL"):
            continue
        for f in refs(ent[shell]):
            if kind.get(f) not in ("ADVANCED_FACE", "FACE_SURFACE"):
                continue
            faces += 1
            surf = refs(ent[f])[-1] if refs(ent[f]) else None
            body = ent.get(surf, "")
            if any(c in body[:60] for c in curved_kinds):
                curved += 1
    return faces, curved


def face_counts(rep, seen):
    if rep in seen:
        return 0, 0
    seen.add(rep)
    f = c = 0
    for r in refs(ent.get(rep, "")):
        if kind.get(r) in solid_kinds:
            a, b = faces_of_solid(r)
            f, c = f + a, c + b
    for other in rel.get(rep, []):
        if "REPRESENTATION" in kind.get(other, ""):
            a, b = face_counts(other, seen)
            f, c = f + a, c + b
    return f, c


def part_faces(pd):
    f = c = 0
    for rep in rep_of_pd.get(pd, []):
        a, b = face_counts(rep, set())
        f, c = f + a, c + b
    return f, c
roots = [pd for pd in pds if not parents[pd] and (children[pd] or True)]
roots = [pd for pd in roots if children[pd]] or roots

# Expand the tree: total occurrences, depth, solids as placed.
occ = Counter()
max_depth = 0
placed_solids = 0
per_depth = Counter()


def walk(pd, depth, stack):
    global max_depth, placed_solids
    if pd in stack:
        return
    max_depth = max(max_depth, depth)
    per_depth[depth] += 1
    placed_solids += solids.get(pd, 0) if not children[pd] else 0
    for c in children[pd]:
        occ[c] += 1
        walk(c, depth + 1, stack | {pd})


for r in roots:
    walk(r, 0, frozenset())
assemblies = [pd for pd in pds if children[pd]]
parts = [pd for pd in pds if not children[pd]]
print(f"file: {len(text) / 1e6:.1f} MB, {len(ent)} entities")
print(f"product definitions: {len(pds)} ({len(assemblies)} assemblies, {len(parts)} parts); roots {len(roots)}")
print(f"assembly links (NAUO): {len(nauo)}; occurrences when expanded: {sum(occ.values())}")
print(f"nesting depth: {max_depth}; occurrences per depth: {dict(sorted(per_depth.items()))}")
reused = [pd for pd, n in occ.items() if n > 1]
print(f"products used more than once: {len(reused)}; most uses of one: {max(occ.values(), default=0)}")
print(f"solids: {counts['MANIFOLD_SOLID_BREP'] + counts['BREP_WITH_VOIDS']} B-reps defined, {placed_solids} placed; parts with no solid: {sum(1 for p in parts if solids.get(p, 0) == 0)}")
print(f"faces: {counts['ADVANCED_FACE']}; edges: {counts['EDGE_CURVE']}; b-spline surfaces: {counts['B_SPLINE_SURFACE_WITH_KNOTS'] + sum(1 for b in ent.values() if b.startswith('(') and 'B_SPLINE_SURFACE' in b)}")
print(f"children per assembly: {sorted(Counter(len(children[a]) for a in assemblies).items())}")

if tree_out:
    order = {}

    def name(pd):
        return order.setdefault(pd, f"P{len(order)}")

    def node(pd, stack):
        nm = name(pd)
        if pd in stack:
            return {"ref": nm}
        out = {"id": nm}
        if children[pd]:
            out["children"] = [node(c, stack | {pd}) for c in children[pd]]
        else:
            out["solids"] = solids.get(pd, 0)
            # Counts only: how many faces its solids have, and how many of those are curved.
            out["faces"], out["curved"] = part_faces(pd)
        return out

    tree = {"roots": [node(r, frozenset()) for r in roots]}
    json.dump(tree, open(tree_out, "w"), indent=1)
    print(f"anonymous tree written to {tree_out}")
