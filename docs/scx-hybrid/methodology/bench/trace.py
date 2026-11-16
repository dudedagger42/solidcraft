import json, sys
sys.stdout.reconfigure(encoding="utf-8")
lim = int(sys.argv[2]) if len(sys.argv) > 2 else 200
for l in open(sys.argv[1], encoding="utf-8"):
    try:
        m = json.loads(l)
    except ValueError:
        continue
    if m.get("type") == "assistant":
        for c in m["message"]["content"]:
            if c.get("type") == "tool_use":
                print("CALL", c["name"].split("__")[-1], json.dumps(c["input"])[:lim])
            elif c.get("type") == "text":
                print("SAY", c["text"][:lim].replace("\n", " "))
    elif m.get("type") == "user":
        for c in m["message"].get("content", []):
            if isinstance(c, dict) and c.get("type") == "tool_result":
                cont = c.get("content")
                t = "".join(x.get("text", "") for x in cont if isinstance(x, dict)) if isinstance(cont, list) else str(cont)
                flag = "ERR " if c.get("is_error") else ""
                print("  <-", flag + str(len(t)), t[:lim].replace("\n", " "))
