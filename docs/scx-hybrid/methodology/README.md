# scx benchmark methodology

This folder holds what you need to rerun the scx benchmarks in [../README.md](../README.md): the harness, the
task generators, the task files, the checkers and the reference builds for the 23 tuning tasks and ladder
levels L1-L7. L8 and L9 used files that cannot be published; they are described in prose below.

## What is compared

Every task is run once per setup, with the same model, prompt, client and checker. Only the MCP server changes.

| Setup | Name in files | Server |
|---|---|---|
| Plain SolidCraft MCP | `sc` | `solidcraft-cli mcp --connect 127.0.0.1:PORT` (21 tools) |
| scx | `hy` | `python -I scx.py --port PORT` (3 tools) |
| scx built in | `hyb` | `solidcraft-cli mcp --connect 127.0.0.1:PORT --scx` (3 tools): the same vocabulary in SolidCraft's own sandboxed interpreter. The published numbers were measured with `scx.py`; `bench/refcheck.py` checks the two build the same designs. |
| FreeCAD MCP | `fc` | the open-source FreeCAD MCP server, run with `uv`. Only in the first tuning rounds (`ROUNDS.md`). |

The client is Claude Code in print mode. One run is one call like this (from `bench.py`):

```
claude -p "<prompt>" --model <model-id> --output-format stream-json --verbose
  --strict-mcp-config --mcp-config '{"mcpServers": {...one server...}}'
  --allowedTools mcp__solidcraft --setting-sources local
  --disallowedTools Bash,PowerShell,Read,Write,Edit,Glob,Grep,WebFetch,WebSearch,Agent,Task,NotebookEdit
```

- The model can use only the one MCP server. No files, no shell, no web.
- `--setting-sources local` keeps the user's own settings and CLAUDE.md out of the run.
- Each run starts in its own empty working folder.
- The L9 harness also blocks `Skill`, after two runs went off task (see the main README). The L1-L8 runs did
  not block it.
- Time limit: 1,500 s per run (L1-L7), 1,800 s (L8), 2,700 s (L9).

### Model IDs

Always pass the full model ID, never an alias such as `haiku` or `sonnet`. Aliases move to new models. The
scripts take the ID as a required argument, and each `result.json` records it.

| Benchmark | Model ID |
|---|---|
| 23 tuning tasks (v9-v13), ladder L1-L7, L8 | `claude-haiku-5-5` |
| L8 on Sonnet 5 | `claude-sonnet-5` |
| L8 on Opus 5.5 | `claude-opus-5-5` |
| L9 | `claude-sonnet-5-5` |

The earliest tuning rounds (v1-v7, in `ROUNDS.md`) ran on Claude Haiku 4.5 under the `haiku` alias.

To confirm which model really answered, read `message.model` from the `assistant` lines of `stream.jsonl`.

## How a run goes

1. **Reset.** The harness opens a new, empty design in the SolidCraft window (`new_design` over the plain MCP).
2. **Setup (edit tasks only).** The starting design is built by running the task's `setup` script through scx.
   It is the same geometry whatever setup is tested.
3. **Prompt.** The model gets the task text inside a fixed frame (`PROMPT`, `EDIT_PROMPT` in `bench.py`; `FIRST`
   and `NEXT` in `bench_iter.py`). It says: use only the MCP tools, leave no helper solids, do not ask questions.
4. **Thinking report (optional).** With `THINK=1`, the prompt ends with a request for a short written report:
   plan, one line per call, guesses, anything confusing, and one wish. It was never scored. It was used for
   tuning and on all Haiku and Sonnet 5 runs. It was left out on the 5.5 models (`NO_THINK=1` in the L8/L9
   harness), because there it tripped a safety filter at the end of most runs.
5. **Run.** Claude Code runs to the end. Its output goes to `stream.jsonl`.
6. **Check.** The harness reads every body in the design itself (`measure {summary: true}` over the plain
   SolidCraft MCP) and scores it against the expected bodies. The model's own claims are never used.
7. **Record.** Everything goes to `result.json` next to `stream.jsonl`.

Multi-step tasks (L6, L7) run as one conversation: each change request is a new `claude -p ... --resume
<session>` call, and the design is checked after each step. The run stops at the first step that fails.

## How cost, tokens and calls are read

All from `stream.jsonl`, one JSON object per line:

- **Tool calls:** count the `tool_use` blocks in the `assistant` lines.
- **Cost:** `total_cost_usd` from the final `result` line. This is the cost Claude Code computes for the run.
  Under `--resume` (L6, L7) it adds up over the conversation, so `bench_iter.py` takes the value from the last step.
- **Input tokens:** from `usage` in the `result` line: `input_tokens + cache_read_input_tokens +
  cache_creation_input_tokens`. Cache reads count in full here, so this is the context the model carried.
- **Output tokens:** `usage.output_tokens`.
- **Stop reason and final text:** `subtype` and `result` from the `result` line.
- **Wall time:** measured by the harness around the `claude` call.

`trace.py` prints a run as one line per call and per reply, which is how we read what happened.

## How accuracy is checked

Each task file lists the expected bodies: name, volume and bounding box. They were worked out before any run,
in closed form (`mkspecs*.py`, `ladder/volumes.py`) or from a reference build in SolidCraft (`ladder/ref*`).

`score()` in `bench.py`:

- Each expected body is matched to a different found body: the closest by volume and bounding box.
- **Exact:** volume within 0.1% and every bounding box coordinate within 0.05 mm.
- **Pass:** every expected body is exact, and no extra body is left over.
- Names are reported but not required to match.
- **Close** (volume within 0.5%) is also recorded, for reading only.

The oracles build every task with a known-good script and check that the task file agrees, at tighter
tolerances (0.01% volume, 0.01 mm). That checks the task files and the server, not the model.

## Repeats and how results are summed

- **3 runs per task per setup.** Run-to-run spread was 2-12%, so one run cannot separate a 10% change from noise.
- The three repeats run side by side, one per SolidCraft window, on ports 7879, 7880 and 7881. Each window runs
  `sc` and `hy` in turn.
- Results are means over the three runs: calls, input tokens and cost per task.
- In the ladder, a task counts as failed for a setup if fewer than 2 of its 3 runs pass.
- Tuning rule: keep a change only if the 3-run mean improves cost or tokens by at least 5% with no lost pass.

## Files

### bench/

| File | What it is |
|---|---|
| `bench.py` | Harness for one run of one single-prompt task: reset, setup, prompt, run Claude Code, check, score. |
| `bench_iter.py` | Harness for multi-step tasks (L6, L7): several change requests in one conversation, checked after each. |
| `oracle.py` | Builds every simple, big and relational task with a known-good scx script and checks the task file. |
| `oracle_edit.py` | The same for the 3 edit tasks. |
| `ladder_score.py` | Sums a folder of runs: per task and setup, passes out of 3 and mean calls, input tokens and cost. |
| `trace.py` | Prints one run's calls and replies from its `stream.jsonl`. |
| `refcheck.py` | Runs every L1-L5 reference script through the scx built into SolidCraft (`solidcraft-cli mcp --headless --scx`) and scores the result like `bench.py`. No model, no cost: it checks that the built-in scx builds what `scx.py` built. 15/15 pass. |
| `mkspecs.py` | Writes `specs12.json`: 12 simple tasks with closed-form expected bodies. |
| `mkspecs_big.py` | Writes `specsBig.json`: 4 big tasks (heatsink, board on standoffs, rack, bolted flange pair). |
| `mkspecs_rel.py` | Writes `specsRel.json`: 4 tasks described in words (centred, evenly spaced, stacked). |
| `mkspecs_edit.py` | Writes `specsEdit.json`: 3 edits to a 24-part rack assembly, with the setup script. |
| `specs12.json`, `specsBig.json`, `specsRel.json`, `specsEdit.json` | The 23 tuning tasks. |
| `specsL1.json` ... `specsL5.json` | Ladder L1-L5 tasks, written by `ladder/mkspecs_ladder.py`. |
| `specsL6.json`, `specsL7.json` | Ladder L6 and L7 multi-step tasks, written by `ladder/mk_l6.py` and `ladder/mk_l7.py`. |
| `run_v.sh` | One repeat of the 23 tuning tasks (or of any spec sets) on one SolidCraft window. |
| `run_ladder.sh` | One ladder level L1-L5: 3 tasks x 2 setups x 3 repeats on three windows. |
| `run_ladder6.sh`, `run_ladder7.sh` | L6 and L7 the same way, through `bench_iter.py`. |
| `ROUNDS.md` | Results of the first tuning rounds (FreeCAD MCP, plain MCP, scx v1-v7 on Haiku 4.5), one run per task. |

### ladder/

| File | What it is |
|---|---|
| `tasks.json` | The L1-L5 tasks: text, expected bodies and how each volume was worked out (`math`). |
| `volumes.py` | Closed-form volumes and boxes for L1-L4. `--check` cross-checks them by grid sampling. It also writes a first L5 draft, which was replaced (see below). |
| `tasks_L5_vague.json` | That first L5 draft (tray, idler wheel, pillow blocks). Not used in the published numbers. |
| `mkspecs_ladder.py` | Splits `tasks.json` into `bench/specsL1.json` ... `specsL5.json`. |
| `ref/` | Reference scx scripts for every L1-L5 task. `*_start.py` builds the starting design of an L3 edit task. |
| `ref6/`, `ref7/` | Reference states for L6 and L7: `*_0.py` or `*_start.py` is the start, `_1`, `_2`, ... the state after each step. |
| `mk_l6.py`, `mk_l7.py` | Build each reference state in SolidCraft and record its bodies as the expected result of that step. |
| `ref.py` | Runs one scx script on a SolidCraft window and prints the reply. Used by `mk_l6.py` and `mk_l7.py`. |
| `sc.py` | Calls one plain SolidCraft MCP tool on a window. `python sc.py 7879 list_commands` dumps the command list. |

The L5 tasks in `tasks.json` (duct assembly, three-blade rotor, gearbox) were written after `volumes.py`. Their
`math` field says how each was worked out. Run `volumes.py` and it overwrites `tasks.json` with the draft L5, so
keep a copy.

## Reproduce

### Requirements

- Python 3.10 or later. The checkers use only the standard library.
- SolidCraft 0.40 (the runs used 0.40.0-rc), with `solidcraft` and `solidcraft-cli`. Set `SCX_CLI` if
  `solidcraft-cli` is not on PATH.
- Claude Code (`claude`) logged in. Set `CLAUDE_BIN` if it is not on PATH.
- Bash for the `run_*.sh` scripts (Git Bash works on Windows). Set `PY` if `python` is not the right interpreter.

Environment variables the scripts read:

| Variable | Default | Meaning |
|---|---|---|
| `SC_PORT` | 7879 | SolidCraft control port the run uses |
| `HY_SERVER` | `../scx.py` (next to this folder) | the scx server to test |
| `SETUP_SERVER` | same as `HY_SERVER` | the scx server that builds edit-task setups |
| `SCX_CLI` | `solidcraft-cli` | path to solidcraft-cli |
| `CLAUDE_BIN` | `claude` | path to Claude Code |
| `SPECS`, `RUNS` | `specs12.json`, `runs12` | task file and output folder (under `bench/`) |
| `THINK` | off in `bench.py`, on in `run_*.sh` | add the written Thinking report to the prompt |
| `FC_MCP_SRC` | none | FreeCAD MCP checkout, only for `fc` runs |

### 1. Start three SolidCraft windows

```bash
solidcraft --control 7879 &
solidcraft --control 7880 &
solidcraft --control 7881 &
```

### 2. Optional: rebuild and check the task files

```bash
cd bench
python mkspecs.py && python mkspecs_big.py && python mkspecs_rel.py && python mkspecs_edit.py
SC_PORT=7879 python oracle.py          # every line should say OK
SC_PORT=7879 python oracle_edit.py     # every line should say OK
cd ../ladder
python mkspecs_ladder.py               # specsL1..L5 from tasks.json
SC_PORT=7879 python mk_l6.py           # specsL6 from ref6/ (builds in SolidCraft)
SC_PORT=7879 python mk_l7.py           # specsL7 from ref7/
```

### 3. The 23 tuning tasks, both setups, 3 repeats

```bash
cd bench
for rep in 1 2 3; do
  port=$((7878 + rep))
  ( KIND=sc bash run_v.sh $rep $port claude-haiku-5-5 runsTune
    KIND=hy bash run_v.sh $rep $port claude-haiku-5-5 runsTune ) > logTune-r$rep.txt 2>&1 &
done
wait
python ladder_score.py runsTune
```

One task, one run, by hand:

```bash
cd bench
SPECS=specsBig.json RUNS=runsOne SC_PORT=7879 THINK=1 python -I bench.py hy heatsink claude-haiku-5-5
python trace.py runsOne/heatsink-hy-claude-haiku-5-5/stream.jsonl
```

### 4. Ladder L1-L7

```bash
cd bench
for L in 1 2 3 4 5; do bash run_ladder.sh $L claude-haiku-5-5; done
bash run_ladder6.sh claude-haiku-5-5
bash run_ladder7.sh claude-haiku-5-5
python ladder_score.py runsLadder
```

Results land in `bench/runsLadder/L<level>-r<rep>/<task>-<setup>-<model>/` as `result.json` and `stream.jsonl`.
Finished runs are skipped, so a stopped batch can be restarted.

### Server versions

This folder runs the published `scx.py` (v15.1) by default. The published numbers used:

| Benchmark | scx version |
|---|---|
| tuning rounds | v1-v13, one per round (see the [change log](../CHANGELOG.md)) |
| L1-L6 | v14 |
| L7 | v15 |
| L8, L9 | v15.1 (`scx.py`) |

Only v15.1 is published. To rerun an older round exactly, point `HY_SERVER` at that version.

## L8: a hotend mount in an imported toolhead

L8 used an imported 3D-printer toolhead assembly (a private STEP file). Its geometry, start designs, task text
and checker cannot be published, because they hold the assembly's coordinates and names. This is how it ran.

- **Task.** Six steps turn a blank block into a hotend mount (listed in the main README). Each step is a
  separate run. Each starts from a saved reference solution of the step before, so one miss does not sink the
  rest. The prompt is `EDIT_PROMPT` from `bench.py`, filled with a shared context paragraph (what the design
  holds, which body to change, what must not change) and the step's text.
- **Harness.** Like `bench.py`, plus: load the step's start design, save the result after the run, then run
  the checker. Repeat r1, r2, r3 ran on three windows; odd repeats ran `sc` first and even ones `hy` first.
- **Checker.** It works on the open design:
  - Every provided body must be unchanged: bounding box within 0.05 mm and volume within 0.1% of a snapshot.
  - No body may be added or removed.
  - The mount must be one closed solid inside its original block.
  - For each rule, the checker adds a probe solid (a box or a cylinder) and asks SolidCraft's
    `inspect.clearance` how much of the mount lies inside it. A region that must stay empty fails above
    0.5 mm³ of mount. A region that must keep material fails below 98% filled. A wall around a hole or channel
    is checked as a ring (outer cylinder minus inner) and fails below 85-90% filled.
  - The probes are removed again.
  - If the kernel cannot evaluate a probe pair, the step is "unverified", not failed, and is checked by hand.
- **Models.** `claude-haiku-5-5` (with the Thinking report), `claude-sonnet-5` (with it), `claude-opus-5-5`
  (without it, `NO_THINK=1`). 3 runs per step per setup.

## L9: rebuild a freeform vessel from a mesh

L9's target is a decorative candle vessel that is not ours to publish. Its mesh, its dimensions and the task
text (which gives its size and where its features are) are left out. This is how it ran.

- **Task.** The design holds the target as a mesh body. Three chained steps rebuild it as one solid; each step
  starts from that run's own result of the step before. Steps: outer form; cavity and lip; a lofted relief
  on one side.
- **Harness.** Like the L8 harness, with `Skill` blocked, a 2,700 s limit and `claude-sonnet-5-5` without the
  Thinking report. It records the model seen in `stream.jsonl`.
- **Scorer.** It exports the candidate solid to STL (`file.export_stl`, quality `fine`) and compares it with the
  target mesh in Python with NumPy and SciPy:
  - **Overlap:** both meshes are turned into 0.5 mm voxels (rays along one axis through each column, odd
    number of crossings = inside). The score is intersection over union. Step 1 compares against the mesh
    with its cavity filled (inside = between the first and last crossing).
  - **Relief zone:** the same overlap inside a box around the relief only.
  - **Surface distance:** 200,000 points sampled on each surface by area. For each point, the distance to
    the nearest point of the other surface (KD-tree). The scorer reports the mean and 95th percentile, and from
    step 2 on the mean of both directions.
  - **Flags:** the target must be unchanged, and it must be the only body besides exactly one closed solid.
  - **Scale:** a plain cylinder of the right height and a perfect revolve of the average profile were scored
    the same way. The main README gives their numbers.
- **Reported:** the mean per step and the median of the final overlap over 3 runs per setup.

## What is not here, and why

- **Run outputs** (`stream.jsonl`, `result.json`, logs): bulky, and they hold full conversations.
- **L8 and L9 files** (meshes, STEP, start designs, task text, checker and scorer code, reference volumes):
  they hold private geometry. They are described above.
- **Older scx versions** (v1-v15): only the published `scx.py` (v15.1) is included.
- **The FreeCAD MCP server:** a separate open-source project. `bench.py` still supports it (`fc`), through
  `FC_MCP_SRC`.
- **`commands.txt`** (a 360 kB dump of SolidCraft's command list, used to write tasks): rebuild it with
  `python ladder/sc.py 7879 list_commands`.
- **Drafts and one-off scripts** (unused reference drafts, smoke tests, round tables built from local run
  folders).

## Changes from the scripts as run

The scripts are the ones used, with these changes for publication:

- Local paths became command-line arguments, environment variables or paths relative to this folder.
- The model ID is now a required argument (the first tuning rounds used the `haiku` alias).
- `ref.py` and its v15 twin are one script; the server comes from `HY_SERVER`.
- The edit-task setup server defaults to `HY_SERVER` (it was scx v4; the geometry is the same).
- A few variables were renamed.

The scripts have not been rerun since these changes.
