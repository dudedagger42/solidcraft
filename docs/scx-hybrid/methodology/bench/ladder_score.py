# Ladder table: per task and setup, passes out of 3, mean calls / input tokens / cost; task fails if < 2 of 3.
# usage: python ladder_score.py [RUNS_DIR]   (default: runsLadder next to this file; reads RUNS_DIR/*-r*/*/result.json)
import glob, json, os, statistics as st, sys
HERE = os.path.dirname(os.path.abspath(__file__))
RUNS = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "runsLadder")
rows = {}
for f in glob.glob(os.path.join(RUNS, "*-r*", "*", "result.json")):
    r = json.load(open(f, encoding="utf-8"))
    rows.setdefault((r["part"], r["app"]), []).append(r)
tasks = sorted({p for p, _ in rows})
print(f"{'task':28} {'setup':5} pass  calls  in_k  cents")
streak = {"sc": 0, "hy": 0}
for t in tasks:
    for k in ("sc", "hy"):
        rs = rows.get((t, k), [])
        if not rs: continue
        n = sum(1 for r in rs if r["score"]["pass"])
        m = lambda field: st.mean((r.get(field) or 0) for r in rs)
        print(f"{t:28} {k:5} {n}/{len(rs)}  {m('tool_calls'):5.1f}  {m('input_tokens')/1000:5.0f}  {m('cost_usd')*100:5.1f}")
        if len(rs) == 3:
            streak[k] = streak[k] + 1 if n < 2 else 0
print("fails in a row:", streak)
