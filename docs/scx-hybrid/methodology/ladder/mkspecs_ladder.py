# tasks.json (ladder) -> bench spec files, one per level: specsL1.json .. specsL5.json
import json, os
HERE = os.path.dirname(os.path.abspath(__file__))
tasks = json.load(open(os.path.join(HERE, "tasks.json"), encoding="utf-8"))
by_level = {}
for k, t in tasks.items():
    sp = {"spec": t["spec"], "bodies": t["bodies"]}
    if t.get("start"):
        sp["setup"], sp["edit"] = open(os.path.join(HERE, "ref", k + "_start.py"), encoding="utf-8").read(), True
    by_level.setdefault(t["level"], {})[k] = sp
for lv, d in sorted(by_level.items()):
    p = os.path.join(HERE, "..", "bench", f"specsL{lv}.json")
    json.dump(d, open(p, "w", encoding="utf-8", newline="\n"), indent=1)
    print(p, list(d))
