"""Build every spec with the hybrid vocabulary and compare with specs12.json (checks the specs and the vocabulary)."""
import json, os, sys, subprocess
sys.stdout.reconfigure(encoding="utf-8")
HERE = os.path.dirname(os.path.abspath(__file__))
SPECS = json.load(open(os.path.join(HERE, os.environ.get("SPECS", "specs12.json")), encoding="utf-8"))
SCRIPTS = {
"plate": """box(60,40,8,name='plate')
for x,y in [(8,8),(52,8),(8,32),(52,32)]: hole(at=(x,y,8), d=6)""",
"bracket": """box(80,40,6,name='bracket')
box(80,6,50,at=(0,34,0),op='join')
fillet([(40,34,6)], 5)
s=sketch('XY',offset=6)
s.slot((20,17),20,6); s.slot((60,17),20,6)
s.extrude(-6, op='cut')""",
"enclosure": """box(100,60,40,name='enclosure')
fillet([(0,0,20),(100,0,20),(0,60,20),(100,60,20)],5)
shell([(50,30,40)],2.5)
for x,y in [(8,8),(92,8),(8,52),(92,52)]:
    cylinder(37.5,d=8,at=(x,y,2.5),op='join')
    hole(at=(x,y,40),d=3,depth=30)
hole(at=(50,0,20),d=10,depth=2.5)""",
"flange": """cylinder(10,d=80,name='flange')
cylinder(20,d=40,at=(0,0,10),op='join')
hole(at=(0,0,30),d=20)
for k in range(6):
    a=radians(60*k); hole(at=(30*cos(a),30*sin(a),10),d=8)""",
"shaft": """cylinder(30,d=20,axis='X',name='shaft')
cylinder(40,d=30,at=(30,0,0),axis='X',op='join')
cylinder(25,d=16,at=(70,0,0),axis='X',op='join')
chamfer([(0,10,0),(95,8,0)],1)""",
"spacer": """s=sketch('XY')
s.polygon((0,0),6,12/cos(radians(30)),angle=0)
s.extrude(10,name='spacer')
hole(at=(0,0,10),d=12)""",
"channel": """s=sketch('YZ')
s.poly([(0,0),(40,0),(40,30),(36,30),(36,4),(4,4),(4,30),(0,30)])
s.extrude(50,name='channel')
for x in (15,35): hole(at=(x,20,4),d=6)""",
"rounded": """box(50,30,20,name='rounded')
fillet([e['mid'] for e in edges('rounded')],3)""",
"frustum": """cone(30,40,20,name='frustum')
hole(at=(0,0,30),d=10)""",
"rail": """box(100,20,5,name='rail')
for x in range(15,90,10): hole(at=(x,10,5),d=4)""",
"clamp": """with part('Base'):
    box(60,40,20,name='Base')
    for x in (10,50): hole(at=(x,20,20),d=5,depth=15)
with part('Lid'):
    box(60,40,5,at=(0,0,20),name='Lid')
    for x in (10,50): hole(at=(x,20,25),d=5)
for i,x in enumerate((10,50)):
    with part(f'Pin{i+1}'):
        cylinder(20,d=5,at=(x,20,5),name=f'Pin{i+1}')""",
"bearing": """with part('Housing'):
    box(80,50,40,name='Housing')
    hole(at=(0,25,20),d=30)
    for x,y in [(10,6),(70,6),(10,44),(70,44)]: hole(at=(x,y,40),d=6)
with part('Shaft'):
    cylinder(120,d=30,at=(-20,25,20),axis='X',name='Shaft')
for n,x0 in (('CollarL',-12),('CollarR',82)):
    with part(n):
        s=sketch('YZ',offset=x0); s.circle((25,20),d=40); s.circle((25,20),d=30)
        s.extrude(10,name=n)""",
"bushplate": """box(90,90,10,center=(0,0,5),name='bushplate')
hole(at=(0,0,10),d=30)
for k in range(8):
    a=radians(45*k); hole(at=(33*cos(a),33*sin(a),10),d=6)""",
"slotcube": """box(50,50,50,name='slotcube')
box(30,50,20,at=(10,0,30),op='cut')""",
"tee": """cylinder(100,d=30,at=(-50,0,0),axis='X',name='tee')
cylinder(50,d=30,at=(0,0,0),axis='Y',op='join')
cylinder(100,d=24,at=(-50,0,0),axis='X',op='cut')
cylinder(50,d=24,at=(0,0,0),axis='Y',op='cut')""",
"washers": """with part('Pin'): cylinder(30,d=10,name='Pin')
for k in range(5):
    with part(f'W{k+1}'):
        s=sketch('XY',offset=2*k); s.circle((0,0),d=20); s.circle((0,0),d=10); s.extrude(2,name=f'W{k+1}')""",
"heatsink": """box(80,60,5,name='heatsink')
for i in range(16): box(2,60,25,at=(1.5+5*i,0,5),op='join')""",
"standoffs": """with part('Board'):
    box(100,80,1.6,at=(0,0,10),name='Board')
    for y in (10,40,70):
        for x in (10,30,50,70,90): hole(at=(x,y,11.6),d=3.2)
i=0
for y in (10,40,70):
    for x in (10,30,50,70,90):
        i+=1
        with part(f'S{i}'):
            cylinder(10,d=6,at=(x,y,0),name=f'S{i}')
            hole(at=(x,y,10),d=3)""",
"rack": """for i,(x,y) in enumerate([(0,0),(280,0),(0,180),(280,180)]):
    with part(f'P{i+1}'): box(20,20,400,at=(x,y,0),name=f'P{i+1}')
for i,z in enumerate((50,150,250,350)):
    with part(f'Shelf{i+1}'):
        box(300,200,10,at=(0,0,z),name=f'Shelf{i+1}')
        for x,y in [(0,0),(280,0),(0,180),(280,180)]: box(20,20,10,at=(x,y,z),op='cut')""",
"flangepair": """for n,z in (('FlangeA',0),('FlangeB',14)):
    with part(n):
        cylinder(12,d=120,at=(0,0,z),name=n)
        hole(at=(0,0,z+12),d=50)
        for k in range(8):
            a=radians(45*k); hole(at=(47.5*cos(a),47.5*sin(a),z+12),d=10)
with part('Gasket'):
    s=sketch('XY',offset=12); s.circle((0,0),d=80); s.circle((0,0),d=50); s.extrude(2,name='Gasket')
for k in range(8):
    a=radians(45*k)
    with part(f'B{k+1}'): cylinder(36,d=10,at=(47.5*cos(a),47.5*sin(a),-5),name=f'B{k+1}')""",
}
p = subprocess.Popen([sys.executable, "-I", os.environ.get("HY_SERVER", os.path.join(HERE, "..", "..", "scx.py")), "--port", os.environ.get("SC_PORT", "7879")], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, encoding="utf-8")
n = 0


def rpc(method, params):
    global n
    n += 1
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": n, "method": method, "params": params}) + "\n")
    p.stdin.flush()
    return json.loads(p.stdout.readline())


def run(code):
    r = rpc("tools/call", {"name": "run", "arguments": {"code": code}})["result"]
    return r["content"][0]["text"], r["isError"]


rpc("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "o", "version": "0"}})
only = sys.argv[1:] or [k for k in SCRIPTS if k in SPECS]
for k in only:
    run("new('Oracle')")
    t, err = run(SCRIPTS[k])
    m = json.loads(run("print(__import__) if False else None\nimport_ok=1")[0] and "{}") if False else None
    got, _ = run("import_ok=1\nm=measure()\nprint(m)")
    got = eval(got.splitlines()[0])
    want = {b["name"]: b for b in SPECS[k]["bodies"]}
    bad = []
    for name, w in want.items():
        g = got.get(name)
        if not g:
            bad.append(f"{name} missing (have {list(got)})")
            continue
        dv = abs(g["volume"] - w["volume"]) / w["volume"]
        db = max(abs(a - b) for pp, qq in zip(g["bbox"], w["bbox"]) for a, b in zip(pp, qq))
        if dv > 1e-4 or db > 0.01:
            bad.append(f"{name}: vol {g['volume']} want {w['volume']} bbox {g['bbox']} want {w['bbox']}")
    extra = set(got) - set(want)
    print(k, "OK" if not bad and not extra else f"BAD {bad} extra {extra}", "| script error" if err else "", "" if not err else t[:300])
