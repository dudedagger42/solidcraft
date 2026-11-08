"""Apply reference edits to the setup design with the hybrid server and score them against specsEdit.json."""
import json, os, subprocess, sys
sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bench

EDITS = {
    "edit_move": """move(['Board'] + [f'S{i}' for i in range(1,16)], by=(0,0,100))
move('Shelf4', by=(0,0,20))""",
    "edit_add": """for i in range(1,5): remove(f'P{i}')
for i,(x,y) in enumerate([(0,0),(280,0),(0,180),(280,180)]):
    with part(f'P{i+1}b'): box(20,20,450,at=(x,y,0),name=f'P{i+1}')
with part('Shelf5'):
    box(300,200,10,at=(0,0,430),name='Shelf5')
    for x,y in [(0,0),(280,0),(0,180),(280,180)]: box(20,20,10,at=(x,y,430),op='cut')""",
    "edit_holes": """for i in range(1,5): hole(at=(150,170,50+100*(i-1)+10), d=6, depth=10)
remove(['S1','S15'])""",
}
specs = json.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "specsEdit.json"), encoding="utf-8"))
for k, code in EDITS.items():
    bench.reset("sc")
    bench.setup_design(specs[k]["setup"])
    bench.setup_design(code)
    sc = bench.score(bench.check("sc"), specs[k]["bodies"])
    print(k, "OK" if sc["pass"] else ("BAD", [r for r in sc["rows"] if not r.get("exact")], "extra", sc["extra"]))
