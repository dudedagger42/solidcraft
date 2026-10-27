"""macOS reproduction for a tangency-labs/solidcraft issue, run by .github/workflows/mac-repro.yml.

    python3 -I replay.py fetch N WORK        download issue N's attachments into WORK (needs GH_TOKEN for the API)
    python3 -I replay.py run N WORK          run them (and the SCRIPT env var, if set) on $SOLIDCRAFT_CLI; no token

Prints lines like the Windows kit's repro/replay.py: REPLAY #N <result> | file | detail, with
FAILED / RAN_CLEAN / NO_FILE / CANNOT_RUN, and SCRIPT #N <result> | ... for the given script.
Downloaded files are untrusted data: they are never executed, only passed to the CLI as arguments.
"""
import glob, json, os, re, subprocess, sys, urllib.request, zipfile

ATT = re.compile(r"https://github\.com/user-attachments/files/[^\s)\"\]>]+")
RUNNABLE = (".solidcraft", ".json", ".step", ".stp")
REPO = "tangency-labs/solidcraft"


def get(url, token=None, limit=60_000_000):
    h = {"User-Agent": "mac-repro"}
    if token:
        h["Authorization"] = f"Bearer {token}"
    with urllib.request.urlopen(urllib.request.Request(url, headers=h), timeout=60) as r:
        data = r.read(limit + 1)
    if len(data) > limit:
        raise RuntimeError("over 60 MB")
    return data


def fetch(n, work):
    tok = os.environ.get("GH_TOKEN")
    api = f"https://api.github.com/repos/{REPO}/issues/{n}"
    texts = [json.loads(get(api, tok)).get("body") or ""]
    texts += [c.get("body") or "" for c in json.loads(get(api + "/comments?per_page=100", tok))]
    urls = list(dict.fromkeys(u for t in texts for u in ATT.findall(t)))
    os.makedirs(work, exist_ok=True)
    got = []
    for i, u in enumerate(urls[:6]):
        name = re.sub(r"[^\w.\-]", "_", u.rsplit("/", 1)[-1])[:80] or f"att{i}"
        dest = os.path.join(work, f"{i}-{name}")
        try:
            open(dest, "wb").write(get(u))  # attachments are public: no token sent
            got.append(dest)
        except Exception as e:
            print(f"download failed: {u} ({str(e)[:100]})")
    print(f"#{n}: {len(urls)} attachment(s), {len(got)} downloaded")


def runnable(work):
    files = []
    for dest in sorted(glob.glob(os.path.join(work, "*"))):
        if zipfile.is_zipfile(dest):
            out = dest + "_x"
            with zipfile.ZipFile(dest) as z:
                for m in z.namelist()[:200]:  # zip-slip safe: only members that stay inside `out`
                    t = os.path.realpath(os.path.join(out, m))
                    if t.startswith(os.path.realpath(out) + os.sep) and not m.endswith("/") and z.getinfo(m).file_size < 200_000_000:
                        os.makedirs(os.path.dirname(t), exist_ok=True)
                        open(t, "wb").write(z.read(m))
            files += [p for p in glob.glob(os.path.join(out, "**", "*"), recursive=True) if p.lower().endswith(RUNNABLE)]
        elif dest.lower().endswith(RUNNABLE):
            files.append(dest)
    return files


def is_script(f):
    try:
        d = json.load(open(f, encoding="utf-8"))
        return isinstance(d, dict) and "commands" in d
    except Exception:
        return False


def cli(cmd, out_json):
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=300)
    except subprocess.TimeoutExpired:
        return "CANNOT_RUN", "timed out after 300 s (does it hang?)"
    text = (r.stderr or r.stdout or "").strip().replace("\n", " ")[:400]
    if r.returncode != 0:
        if r.returncode < 0:
            return "FAILED", f"crashed (signal {-r.returncode}) | {text}"
        unreadable = re.search(r"invalid utf-8|unsupported|not a valid|cannot read|unknown format", text, re.I)
        return ("CANNOT_RUN" if unreadable else "FAILED"), f"exit {r.returncode} | {text}"
    errs = []
    try:
        d = json.load(open(out_json, encoding="utf-8"))
        errs = [(t.get("name"), t.get("error")) for t in d.get("timeline", []) if t.get("error")] if isinstance(d, dict) else []
    except Exception:
        pass
    if errs:
        return "FAILED", f"{len(errs)} feature(s) with an error: " + "; ".join(f"{a}: {str(b)[:120]}" for a, b in errs[:3])
    return "RAN_CLEAN", "no feature reports an error"


def run(n, work):
    exe = os.environ["SOLIDCRAFT_CLI"]
    res = os.path.join(work, "results")
    os.makedirs(res, exist_ok=True)
    lines = []
    files = runnable(work)
    designs = [f for f in files if not f.lower().endswith(".json")]
    if not files:
        lines.append(f"REPLAY #{n} NO_FILE | nothing runnable attached")
    for i, f in enumerate(files[:3]):
        oj = os.path.join(res, f"replay-{i}.json")
        if f.lower().endswith(".json") and not is_script(f):
            lines.append(f"REPLAY #{n} NO_FILE | {os.path.basename(f)} | a .json that is not a command script")
            continue
        cmd = [exe, "run", f, "--results", "--json-out", oj] if f.lower().endswith(".json") else [exe, "eval", f, "--json-out", oj]
        r, d = cli(cmd, oj)
        lines.append(f"REPLAY #{n} {r} | {os.path.basename(f)} | {d}")
    script = os.environ.get("SCRIPT", "").strip()
    if script:
        sp = os.path.join(res, "script.json")
        open(sp, "w", encoding="utf-8").write(script)
        oj = os.path.join(res, "script-result.json")
        cmd = [exe, "run", sp, "--results", "--json-out", oj] + (["--in", designs[0]] if designs else [])
        r, d = cli(cmd, oj)
        lines.append(f"SCRIPT #{n} {r} | {'on ' + os.path.basename(designs[0]) if designs else 'standalone'} | {d}")
    ver = subprocess.run([exe, "--version"], capture_output=True, text=True).stdout.strip()
    osv = subprocess.run(["sw_vers", "-productVersion"], capture_output=True, text=True).stdout.strip()
    arch = subprocess.run(["uname", "-m"], capture_output=True, text=True).stdout.strip()
    lines.append(f"(official CLI: {ver}; macOS {osv} {arch}, GitHub Actions runner, no GPU window)")
    out = "\n".join(lines)
    print(out)
    open(os.path.join(res, "summary.txt"), "w", encoding="utf-8").write(out + "\n")
    if os.environ.get("GITHUB_STEP_SUMMARY"):
        open(os.environ["GITHUB_STEP_SUMMARY"], "a", encoding="utf-8").write("```\n" + out + "\n```\n")


if __name__ == "__main__":
    mode, n, work = sys.argv[1], sys.argv[2], sys.argv[3]
    if not n.isdigit():
        sys.exit("issue must be a number")
    {"fetch": fetch, "run": run}[mode](int(n), work)
