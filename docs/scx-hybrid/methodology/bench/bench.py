"""A model builds a part through one MCP setup; we check the result ourselves.

usage: python -I bench.py <fc|sc|hy> <part> <model-id>
  fc = FreeCAD MCP, sc = plain SolidCraft MCP, hy = scx. Pass the full model id (for example claude-haiku-5-5).
env: SPECS (spec file, default specs12.json), RUNS (output dir, default runs12), SC_PORT (SolidCraft control port,
  default 7879), HY_SERVER (scx server, default ../../scx.py), SCX_CLI (solidcraft-cli), CLAUDE_BIN (claude),
  FC_MCP_SRC (FreeCAD MCP checkout, only for fc), THINK=1 (ask for the written "Thinking:" report).
"""
import json, math, os, subprocess, sys, time, xmlrpc.client

HERE = os.path.dirname(os.path.abspath(__file__))
CLAUDE = os.environ.get("CLAUDE_BIN", "claude")
SC_CLI = os.environ.get("SCX_CLI", "solidcraft-cli")
SC_PORT = int(os.environ.get("SC_PORT", "7879"))
FC_SRC = os.environ.get("FC_MCP_SRC", "")
HY_SERVER = os.environ.get("HY_SERVER", os.path.join(HERE, "..", "..", "scx.py"))

MCP = {
    "fc": {"freecad": {"command": "uv", "args": ["run", "--frozen", "--directory", FC_SRC, "freecad-mcp"]}},
    "sc": {"solidcraft": {"command": SC_CLI, "args": ["mcp", "--connect", f"127.0.0.1:{SC_PORT}"]}},
    "hy": {"solidcraft": {"command": sys.executable, "args": ["-I", HY_SERVER, "--port", str(SC_PORT)]}},
    # scx built into SolidCraft: the same vocabulary, run by solidcraft-cli itself in its sandbox.
    "hyb": {"solidcraft": {"command": SC_CLI, "args": ["mcp", "--connect", f"127.0.0.1:{SC_PORT}", "--scx"]}},
}
APP = {"fc": ("FreeCAD", "freecad"), "sc": ("SolidCraft", "solidcraft"), "hy": ("SolidCraft", "solidcraft"), "hyb": ("SolidCraft", "solidcraft")}

PROMPT = """Using only the {app} MCP tools available to you, build this part in {app}:

{spec}

Work in a new, empty {doc}. When you are done the design must hold only the finished solid(s) the description asks for, with no leftover helper solids. Finish by stating the volume(s) you measured. Don't ask me questions; make reasonable choices."""


EDIT_PROMPT = """Using only the {app} MCP tools available to you, work on the design that is already open in {app}.

{spec}

When you are done the design must hold exactly the solids the description implies, with no leftover helper solids. Finish by stating what you changed. Don't ask me questions; make reasonable choices."""

THINK = """

After that, write a section that starts with the line "Thinking:" and has these lines:
- Plan: what you set out to do, before the first call.
- Each call: one line per tool call: what you meant it to do, and what the reply told you.
- Guessed: any point where you guessed how a tool or argument works, and what you guessed.
- Confusing: anything in the tool descriptions or replies that was unclear, misleading or missing.
- Wish: one thing the tools could do differently that would have saved you a call."""

# Builds the starting design of edit tasks. The original runs used scx v4 here; any later scx builds the same geometry.
SETUP_SERVER = os.environ.get("SETUP_SERVER", HY_SERVER)


def setup_design(code):
    """Build the starting design for an edit spec through the hybrid server (same geometry whatever setup is tested)."""
    p = subprocess.Popen([sys.executable, "-I", SETUP_SERVER, "--port", str(SC_PORT)], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                         stderr=subprocess.DEVNULL, text=True, encoding="utf-8")
    for i, (meth, prm) in enumerate([("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "setup", "version": "0"}}),
                                     ("tools/call", {"name": "run", "arguments": {"code": code}})]):
        p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i, "method": meth, "params": prm}) + "\n")
        p.stdin.flush()
        r = json.loads(p.stdout.readline())
    p.stdin.close()
    p.wait(30)
    if r["result"]["isError"]:
        raise RuntimeError("setup failed: " + r["result"]["content"][0]["text"])


def sc_call(tool, args):
    """One MCP tool call on the SolidCraft MCP (stdio), for reset and checking."""
    req = [
        {"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "bench", "version": "0"}}},
        {"jsonrpc": "2.0", "method": "notifications/initialized"},
        {"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": tool, "arguments": args}},
    ]
    p = subprocess.Popen([SC_CLI, "mcp", "--connect", f"127.0.0.1:{SC_PORT}"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                         stderr=subprocess.DEVNULL, text=True, encoding="utf-8")
    for r in req:
        p.stdin.write(json.dumps(r) + "\n")
    p.stdin.flush()
    while True:
        line = p.stdout.readline()
        if not line:
            raise RuntimeError("SolidCraft MCP closed")
        m = json.loads(line)
        if m.get("id") == 1:
            p.stdin.close()
            p.wait(10)
            return "".join(c.get("text", "") for c in m["result"].get("content", []) if c.get("type") == "text")


FC_CHECK = r"""
import FreeCAD as App, json
out = []
for doc in App.listDocuments().values():
    for o in doc.Objects:
        s = getattr(o, 'Shape', None)
        if s is None or s.isNull() or not s.Solids:
            continue
        if any(getattr(d, 'Shape', None) is not None for d in o.InList):
            continue  # consumed by a later feature
        if not o.Visibility:
            continue  # hidden leftovers don't count, as in the GUI
        b = s.BoundBox
        out.append({'name': o.Label, 'solids': len(s.Solids), 'volume': s.Volume,
                    'bbox': [[b.XMin, b.YMin, b.ZMin], [b.XMax, b.YMax, b.ZMax]]})
print('BENCH' + json.dumps(out))
"""


def fc_rpc():
    return xmlrpc.client.ServerProxy("http://127.0.0.1:9875", allow_none=True)


def reset(kind):
    if kind == "fc":
        fc_rpc().execute_code("import FreeCAD as App\nfor n in list(App.listDocuments()):\n    App.closeDocument(n)\n")
    else:
        sc_call("new_design", {"name": "Bench"})


def check(kind):
    """Leaf solids: name, solids, volume, bbox."""
    if kind == "fc":
        r = fc_rpc().execute_code(FC_CHECK)
        msg = r.get("message", "") if isinstance(r, dict) else str(r)
        return json.loads(msg.split("BENCH", 1)[1].strip().splitlines()[0])
    m = json.loads(sc_call("measure", {"summary": True}))
    return [{"name": b["name"], "solids": 1, "volume": b.get("volume_mm3"), "bbox": [b["bbox"]["min"], b["bbox"]["max"]]}
            for b in m.get("bodies", []) if b.get("volume_mm3")]


def score(found, want):
    """Match each expected body to a distinct found body; exact = vol within 0.1% and bbox within 0.05 mm."""
    left = list(found)
    rows, exact, close = [], 0, 0
    for w in want:
        best = None
        for f in left:
            if not f.get("volume"):
                continue
            dv = abs(f["volume"] - w["volume"]) / w["volume"]
            db = max(abs(a - b) for p, q in zip(f["bbox"], w["bbox"]) for a, b in zip(p, q))
            rank = (dv > 0.001 or db > 0.05, dv + db / 100)
            if best is None or rank < best[0]:
                best = (rank, dv, db, f)
        if best is None:
            rows.append({"want": w["name"], "found": None})
            continue
        _, dv, db, f = best
        left.remove(f)
        ex = dv <= 0.001 and db <= 0.05
        exact += ex
        close += dv <= 0.005 and db <= 0.05
        rows.append({"want": w["name"], "found": f["name"], "vol_err_pct": round(dv * 100, 3), "bbox_err": round(db, 3), "exact": ex})
    return {"pass": exact == len(want) and not left, "exact": exact, "close": close, "of": len(want), "extra": len(left), "rows": rows}


def main():
    if len(sys.argv) < 4:
        raise SystemExit(__doc__)
    kind, part, model = sys.argv[1], sys.argv[2], sys.argv[3]
    specs = json.load(open(os.path.join(HERE, os.environ.get("SPECS", "specs12.json")), encoding="utf-8"))
    sp = specs[part]
    app, srv = APP[kind]
    out = os.path.join(HERE, os.environ.get("RUNS", "runs12"), f"{part}-{kind}-{model}")
    os.makedirs(out, exist_ok=True)
    work = os.path.join(out, "cwd")
    os.makedirs(work, exist_ok=True)
    reset(kind)
    if sp.get("setup"):
        setup_design(sp["setup"])
    prompt = (EDIT_PROMPT if sp.get("edit") else PROMPT).format(app=app, spec=sp["spec"], doc="document" if kind == "fc" else "design")
    if os.environ.get("THINK"):
        prompt += THINK
    cmd = [CLAUDE, "-p", prompt, "--model", model, "--output-format", "stream-json", "--verbose",
           "--strict-mcp-config", "--mcp-config", json.dumps({"mcpServers": MCP[kind]}),
           "--allowedTools", f"mcp__{srv}", "--setting-sources", "local",
           "--disallowedTools", "Bash,PowerShell,Read,Write,Edit,Glob,Grep,WebFetch,WebSearch,Agent,Task,NotebookEdit"]
    t0 = time.time()
    with open(os.path.join(out, "stream.jsonl"), "w", encoding="utf-8") as f:
        try:
            subprocess.run(cmd, cwd=work, stdout=f, stderr=subprocess.STDOUT, timeout=1500)
        except subprocess.TimeoutExpired:
            pass
    wall = time.time() - t0
    final, calls, tool_chars = None, 0, 0
    for line in open(os.path.join(out, "stream.jsonl"), encoding="utf-8"):
        try:
            m = json.loads(line)
        except ValueError:
            continue
        if m.get("type") == "assistant":
            calls += sum(1 for c in m["message"]["content"] if c.get("type") == "tool_use")
        if m.get("type") == "user":
            for c in m["message"].get("content", []):
                if isinstance(c, dict) and c.get("type") == "tool_result":
                    for x in c.get("content", []) if isinstance(c.get("content"), list) else [{"text": str(c.get("content"))}]:
                        tool_chars += len(x.get("text", "")) if x.get("type", "text") == "text" else 0
        if m.get("type") == "result":
            final = m
    found = check(kind)
    u = (final or {}).get("usage", {})
    res = {
        "part": part, "app": kind, "model": model, "wall_s": round(wall), "tool_calls": calls,
        "tool_result_chars": tool_chars, "turns": (final or {}).get("num_turns"),
        "input_tokens": u.get("input_tokens", 0) + u.get("cache_read_input_tokens", 0) + u.get("cache_creation_input_tokens", 0),
        "output_tokens": u.get("output_tokens"), "cost_usd": (final or {}).get("total_cost_usd"),
        "stop": (final or {}).get("subtype"), "claimed": ((final or {}).get("result") or "")[-300:], "final_text": (final or {}).get("result") or "",
        "expected": sp["bodies"], "found": found, "score": score(found, sp["bodies"]),
    }
    json.dump(res, open(os.path.join(out, "result.json"), "w", encoding="utf-8"), indent=1)
    sc = res["score"]; print(json.dumps({**{k: res[k] for k in ("part", "app", "wall_s", "tool_calls", "input_tokens", "output_tokens", "cost_usd")}, "pass": sc["pass"], "exact": f"{sc['exact']}/{sc['of']}", "extra": sc["extra"]}))


if __name__ == "__main__":
    main()
