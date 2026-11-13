"""Iterative ladder task: one conversation, several change requests in turn; the design is checked after each.

usage: python -I bench_iter.py <sc|hy> <task> <model-id>   (env as bench.py: SPECS, RUNS, SC_PORT, HY_SERVER, THINK)
spec: {"setup": scx code, "steps": [{"prompt": text, "bodies": [...]}, ...]}. Stops at the first step that misses.
"""
import json, os, subprocess, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bench as B

FIRST = """Using only the {app} MCP tools available to you, work on the design that is already open in {app}. I will ask for several changes, one at a time.

{spec}

When you are done the design must hold exactly the solids the description implies, with no leftover helper solids. Finish by stating what you changed. Don't ask me questions; make reasonable choices."""
NEXT = """Next change:

{spec}

Change nothing else. Finish by stating what you changed."""


def main():
    if len(sys.argv) < 4:
        raise SystemExit(__doc__)
    kind, task, model = sys.argv[1], sys.argv[2], sys.argv[3]
    sp = json.load(open(os.path.join(B.HERE, os.environ["SPECS"]), encoding="utf-8"))[task]
    app, srv = B.APP[kind]
    out = os.path.join(B.HERE, os.environ["RUNS"], f"{task}-{kind}-{model}")
    work = os.path.join(out, "cwd"); os.makedirs(work, exist_ok=True)
    B.reset(kind)
    B.setup_design(sp["setup"])
    sid, steps = None, []
    for i, st in enumerate(sp["steps"]):
        prompt = (FIRST if i == 0 else NEXT).format(app=app, spec=st["prompt"])
        if os.environ.get("THINK") and i == len(sp["steps"]) - 1:
            prompt += B.THINK
        cmd = [B.CLAUDE, "-p", prompt, "--model", model, "--output-format", "stream-json", "--verbose",
               "--strict-mcp-config", "--mcp-config", json.dumps({"mcpServers": B.MCP[kind]}),
               "--allowedTools", f"mcp__{srv}", "--setting-sources", "local",
               "--disallowedTools", "Bash,PowerShell,Read,Write,Edit,Glob,Grep,WebFetch,WebSearch,Agent,Task,NotebookEdit"]
        if sid:
            cmd += ["--resume", sid]
        t0 = time.time()
        path = os.path.join(out, f"stream{i + 1}.jsonl")
        with open(path, "w", encoding="utf-8") as f:
            try:
                subprocess.run(cmd, cwd=work, stdout=f, stderr=subprocess.STDOUT, timeout=1500)
            except subprocess.TimeoutExpired:
                pass
        final, calls = None, 0
        for line in open(path, encoding="utf-8"):
            try:
                m = json.loads(line)
            except ValueError:
                continue
            if m.get("session_id"):
                sid = m["session_id"]
            if m.get("type") == "assistant":
                calls += sum(1 for c in m["message"]["content"] if c.get("type") == "tool_use")
            if m.get("type") == "result":
                final = m
        u = (final or {}).get("usage", {})
        found = B.check(kind)
        sc = B.score(found, st["bodies"])
        steps.append({"step": i + 1, "wall_s": round(time.time() - t0), "tool_calls": calls,
                      "input_tokens": u.get("input_tokens", 0) + u.get("cache_read_input_tokens", 0) + u.get("cache_creation_input_tokens", 0),
                      "output_tokens": u.get("output_tokens"), "cost_usd": (final or {}).get("total_cost_usd"),
                      "stop": (final or {}).get("subtype"), "final_text": (final or {}).get("result") or "",
                      "found": found, "score": sc})
        print(json.dumps({"task": task, "app": kind, "step": i + 1, "pass": sc["pass"], "calls": calls, "cost": steps[-1]["cost_usd"]}))
        if not sc["pass"]:
            break
    ok = sum(1 for s in steps if s["score"]["pass"])
    res = {"part": task, "app": kind, "model": model, "steps_passed": ok, "of": len(sp["steps"]),
           "score": {"pass": ok == len(sp["steps"])},
           "tool_calls": sum(s["tool_calls"] for s in steps), "input_tokens": sum(s["input_tokens"] for s in steps),
           "cost_usd": max((s["cost_usd"] or 0) for s in steps), "steps": steps}  # total_cost_usd is cumulative under --resume
    json.dump(res, open(os.path.join(out, "result.json"), "w", encoding="utf-8"), indent=1)


if __name__ == "__main__":
    main()
