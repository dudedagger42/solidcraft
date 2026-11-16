# Build a script through scx on a bench window and print the reply. usage: ref.py PORT FILE.py [keep]
# Starts a new design unless "keep" is given. env HY_SERVER: the scx server (default ../../scx.py; L1-L6 used v14, L7 v15).
import json, os, subprocess, sys
SERVER = os.environ.get("HY_SERVER", os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "scx.py"))
port, path = sys.argv[1], sys.argv[2]
code = open(path, encoding="utf-8").read()
p = subprocess.Popen([sys.executable, "-I", SERVER, "--port", port],
                     stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, encoding="utf-8")
def rpc(i, m, prm):
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i, "method": m, "params": prm}) + "\n"); p.stdin.flush()
    while True:
        r = json.loads(p.stdout.readline())
        if r.get("id") == i: return r
rpc(1, "initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "ref", "version": "0"}})
p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
r = rpc(2, "tools/call", {"name": "run", "arguments": {"code": code, "new": len(sys.argv) < 4 or sys.argv[3] != "keep"}})
for c in r["result"]["content"]:
    if c["type"] == "text": print(c["text"])
p.stdin.close(); p.wait()
