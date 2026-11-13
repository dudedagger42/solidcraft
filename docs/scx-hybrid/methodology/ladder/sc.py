# Call one plain SolidCraft MCP tool on a bench window. usage: sc.py PORT TOOL '{json args}'
import json, os, subprocess, sys
p = subprocess.Popen([os.environ.get("SCX_CLI", "solidcraft-cli"), "mcp", "--connect", "127.0.0.1:" + sys.argv[1]], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, encoding="utf-8")
def rpc(i, m, prm):
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i, "method": m, "params": prm}) + "\n"); p.stdin.flush()
    while True:
        r = json.loads(p.stdout.readline())
        if r.get("id") == i: return r
rpc(1, "initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "ref", "version": "0"}})
p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
r = rpc(2, "tools/call", {"name": sys.argv[2], "arguments": json.loads(sys.argv[3] if len(sys.argv) > 3 else "{}")})
for c in r.get("result", {}).get("content", []):
    if c["type"] == "text": print(c["text"])
if "error" in r: print(r["error"])
p.stdin.close(); p.wait()
