"""Checks the scx built into SolidCraft (`solidcraft-cli mcp --scx`) without a model: runs every L1-L5
reference script (ladder/ref) through a headless server and scores the design like bench.py.
The reference scripts build each final design from scratch, so edit tasks need no setup here.

    python refcheck.py [path/to/solidcraft-cli]
"""
import json, os, subprocess, sys
M = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EXE = sys.argv[1] if len(sys.argv) > 1 else "solidcraft-cli"
def score(found, want):
    left = list(found); exact = 0; rows = []
    for w in want:
        best = None
        for f in left:
            dv = abs(f["volume"] - w["volume"]) / w["volume"]
            db = max(abs(a - b) for p, q in zip(f["bbox"], w["bbox"]) for a, b in zip(p, q))
            r = (dv > 0.001 or db > 0.05, dv + db / 100)
            if best is None or r < best[0]: best = (r, dv, db, f)
        if not best: rows.append(f"{w['name']}: missing"); continue
        _, dv, db, f = best; left.remove(f); ok = dv <= 0.001 and db <= 0.05; exact += ok
        if not ok: rows.append(f"{w['name']}~{f['name']}: vol {dv*100:.3f}% bbox {db:.3f}")
    return exact == len(want) and not left, rows + [f"extra {f['name']}" for f in left]
p = subprocess.Popen([EXE, 'mcp', '--headless', '--allow-any-path', '--scx'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, encoding='utf-8')
n = 0
def call(name, args):
    global n; n += 1
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": n, "method": "tools/call", "params": {"name": name, "arguments": args}}) + "\n"); p.stdin.flush()
    r = json.loads(p.stdout.readline())["result"]
    return r["content"][0].get("text", ""), r.get("isError", False)
p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {}}) + "\n"); p.stdin.flush(); p.stdout.readline()
tot = ok = 0
for lvl in range(1, 6):
    specs = json.load(open(os.path.join(M, 'bench', f'specsL{lvl}.json'), encoding='utf-8'))
    for task, sp in specs.items():
        ref = os.path.join(M, 'ladder', 'ref', task + '.py')
        if not os.path.exists(ref): print(task, 'no ref'); continue
        call('run', {"code": "None", "new": True})
        t, e = call('run', {"code": open(ref, encoding='utf-8').read()})
        if e: print(task, 'ERROR', t[:400]); tot += 1; continue
        t, e = call('run', {"code": "import json\njson.dumps(measure())"})
        js = t.split('\n')[0]
        try: m = json.loads(json.loads(js)) if js.startswith('"') else json.loads(js)
        except Exception: print(task, 'cannot parse', t[:200]); tot += 1; continue
        found = [{"name": k, "volume": v["volume"], "bbox": v["bbox"]} for k, v in m.items()]
        passed, rows = score(found, sp["bodies"]); tot += 1; ok += passed
        print(task, 'PASS' if passed else 'FAIL', '; '.join(rows))
print(f'{ok}/{tot} reference builds match their task files')
p.kill()
