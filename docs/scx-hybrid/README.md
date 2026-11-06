# scx: a script-first "hybrid" MCP server for SolidCraft

scx lets a language model build CAD parts in [SolidCraft](https://github.com/tangency-labs/solidcraft) by
writing short Python scripts in SolidCraft's own words (`box`, `cylinder`, `sketch`, `hole`, `fillet`,
`part`, ...) instead of calling dozens of separate tools. Every script runs at once in SolidCraft, and every
reply ends with a short list of what changed, with volumes and bounding boxes, so the model can check its
work without another call.

On a 23-task benchmark with Claude Haiku 5.5 (3 runs per task, 69 runs per setup), compared with the plain
SolidCraft MCP server:

| | Plain SolidCraft MCP (21 tools) | scx v13 (3 tools) |
|---|---:|---:|
| Parts built correctly | 69 / 69 | 69 / 69 |
| Tool calls per task | 6.8 | 2.8 (**-58%**) |
| Input tokens per task | 269k | 125k (**-54%**) |
| Cost per task | 45.7 cents | 18.6 cents (**-59%**) |
| Wall time | 100% | about 70% |

Same model, same tasks, same checker; only the MCP server changed. Edits to an existing design gained the
most: 12.6 calls and 74 cents per task on the plain MCP, 23 cents on v13.

This is a prototype for benchmarking, not a product. See [Limits](#limits) before you use it.

## How it works

### Three tools

| Tool | What it does |
|---|---|
| `run {code, new?}` | Runs a Python script. `new: true` (or a name) starts a new, empty design first. Returns anything printed, errors with line numbers and hints, and a summary of what changed. |
| `design` | A compact outline: every body with volume, bounding box and face count, grouped by part. Identical parts are folded into one line. |
| `look {view?}` | A small picture of the model. Left out with `--text-only`. |

Anything else the SolidCraft MCP offers (`execute`, `inspect_design`, `save`, ...) is passed straight
through, so nothing is lost.

### The vocabulary

The server instructions (the text the client gives the model once) hold the whole vocabulary in under
2,000 characters:

```
box(x, y, z, at=min corner | center=(..), op=, name=, into=)
cylinder(h, d=|r=, at=base centre, axis='Z'|'X'|'Y', op=, name=)   cone(...)   sphere(...)
hole(at=point ON a face, d, depth=None (through), direction=None (into the face))
s = sketch('XY'|'XZ'|'YZ', offset=0)
  s.rect  s.rect_center  s.circle  s.slot  s.polygon  s.poly
  s.extrude(dist, op=, symmetric=False)   s.revolve(axis=, angle=360)
fillet([points on edges], r)   chamfer([points on edges], d)
hollow(t, open='top'|...)   shell([points on open faces], t)
combine(target, [tools], op=)   move(bodies, by=, copy=False)   remove(bodies)   rename(body, new)
with part('Name'): ...
measure()   edges(body)   faces(body)
cmd('command.id', **params)   # any SolidCraft command
```

A whole part is usually one call:

```python
box(60, 40, 8, name='Plate')
for x, y in [(8, 8), (52, 8), (8, 32), (52, 32)]:
    hole(at=(x, y, 8), d=6)
```

The reply:

```
+ Plate: vol 18295.2213  bbox [0, 0, 0]..[60, 40, 8]  faces 10
bodies now: 1, total volume 18295.221
```

### What makes it work

Most of the gain did not come from scripting alone. It came from making the tools behave the way the model
already expects, one observed failure at a time:

- **The reply is the check.** Every `run` ends with `+` new, `~` changed (volume before -> after) and `-`
  removed bodies, tagged with their part. Models stopped calling `measure` or `design` after each step.
- **Defaults match the words people use.** `hollow(3, open='top')` instead of box-minus-box. `op='cut'`
  acts on the last body made or changed, so a cut does not slice every body it touches. `slot` takes the
  overall length, not the distance between arc centres.
- **Forgiving inputs.** The script is accepted as `code`, `script`, `command`, `source` or `python`.
  Feature objects work wherever a body name does. `import math` works.
- **Errors keep the work.** Steps before an error stay built, the reply says so, and the hint names the fix
  (for example "Block is a body name, so quote it: 'Block'").
- **Instructions under 2,048 characters.** Claude Code keeps only the first 2,048 characters of an MCP
  server's instructions. Ours were over that from v4 to v12 without our noticing; the rules at the end
  never reached the model. v13 fits.
- **Small replies.** Many identical new bodies (fins, standoffs, washers) are folded into one line, and the
  `design` outline groups identical parts.

## Setup

You need Python 3.10+ and SolidCraft 0.40 (built and tested on 0.40.0-rc). `solidcraft-cli` must be on PATH,
or set `SCX_CLI` to its path.

Against a running SolidCraft window (you watch the model work):

```bash
solidcraft --control 7879
```

```json
{ "mcpServers": { "solidcraft": { "command": "python", "args": ["-I", "/path/to/scx.py", "--port", "7879"] } } }
```

Headless, for scripts and benchmarks (scx starts its own SolidCraft through `solidcraft-cli`):

```json
{ "mcpServers": { "solidcraft": { "command": "python", "args": ["-I", "/path/to/scx.py", "--headless"] } } }
```

Add `--text-only` for clients that cannot show images. Some clients turn image blocks into text, and one
picture became about 200,000 characters of base64 that filled the model's context.

## How we tuned it

We tuned it like a small training loop, with a person in the loop and one change per version.

1. **Benchmark.** 23 tasks in four sets ([`tasks/`](tasks)): 12 simple parts (plate, L-bracket, enclosure,
   flange, stepped shaft, hex spacer, ...), 4 big ones (a 16-fin heatsink, a 16-part board on standoffs, an
   8-part rack, an 11-part bolted flange pair), 4 that test placement relative to other features (pipe tee,
   slotted cube, ...) and 3 edits to an existing rack (move a board with its standoffs, add parts, add holes).
2. **Independent checking.** Every task carries the expected bodies, worked out beforehand from a reference
   build. A task passes only if every expected body is found with volume within 0.1% and bounding box within
   0.05 mm, and nothing extra is left. The model's own claims are never trusted.
3. **Ask the model what happened.** Each run ends with a written `Thinking:` section: the plan, one line per
   tool call (what it meant and what the reply said), anything it guessed, anything confusing, and one
   **Wish**: what the tools could have done to save a call. This was the most useful signal we had. For
   example, Haiku said in almost every run that the instructions were cut off at "act on the las...". They
   were.
4. **Change one thing, only from evidence.** A change needs a trigger seen in more than one task: a repeated
   error, a repeated Wish, or a wasted call pattern. Each version records its trigger, the exact change and
   the result (see the [change log](CHANGELOG.md)).
5. **Measure with repeats.** Three runs per task. Run-to-run spread was 2-12%, so one run cannot separate a
   10% change from noise.
6. **Stopping rule.** Keep a change only if the 3-run average improves cost or tokens by at least 5% with no
   lost pass. Stop after two changes in a row miss that bar. For Haiku 5.5 that happened at v12 (a
   regression, reverted) and v13 (flat), so v13 is the final Haiku version.

What we learned:

- Gains came from matching what the model already expects (names, words, defaults), not from more
  instruction text. Instruction text is paid for on every call.
- Some well-meant changes made things worse. A "words -> calls" table (v6) cost 30% more tokens and gained no
  pass. Letting `remove()` take part names (v12) doubled edit costs, because bodies and parts shared names.
  The log of triggers made both easy to undo.
- Check what the client actually delivers, not just your source file.

## Version history

The full table with numbers is in [CHANGELOG.md](CHANGELOG.md).

| Version | Change | Result |
|---|---|---|
| v1 | One `run` tool for Python scripts, automatic body diff | 8/12 simple, half the tokens of the plain MCP |
| v2 | Script accepted under several argument names; `slot` takes overall length | 19/20 |
| v3 | `remove()`, last expression printed, identical bodies grouped | 21/23 |
| v4 | `hollow()`, compact `design` outline | 20/23, fewer tokens |
| v5 | cut/join act on the last body; feature objects as names | **23/23**, best of the Haiku 4.5 rounds |
| v6 | "words -> calls" table, cylinder start/end | +30% tokens, no gain: reverted |
| v7 | Error and naming fixes | slips elsewhere: tuning paused |
| v8 | Headless mode, pass-through tools | plumbing for other benches |
| v9 | `run(new=true)` starts a new design | Haiku 5.5: calls 3.5 -> 2.5, input -24% |
| v10 | Replies tag each body with its part | flat; edits a little better |
| v11 | `import math`, `measure(feature)` | two repeated errors gone |
| v12 | `remove()` takes parts, `move(part=)` | edits cost doubled: reverted |
| v13 | Instructions rewritten under 2,048 characters | complaints gone, steadier edits: **final for Haiku** |
| v14 | `--text-only`, hint for unquoted body names | fixes text-only clients |

## Limits

- **Not a sandbox.** Scripts run in the server's own Python process with restricted builtins. That keeps
  honest models on track; it does not stop a hostile script. Do not point it at untrusted prompts.
- In Claude Code the three tools load deferred, so each session spends one ToolSearch call before the first
  `run`. That is a client setting, not the server.
- Known issue: `hollow(open='top')` picks the face at the centre of the top of the bounding box, so it fails
  when something sticks up from that face.
- Tuned and measured on Claude Haiku only. Larger models are untested here.

## License

Like SolidCraft, dual-licensed under [MIT](../../LICENSE-MIT) or [Apache-2.0](../../LICENSE-APACHE), at your
option.

## Credits

Built by Gabe ([@dudedagger42](https://github.com/dudedagger42)) with Claude, on top of SolidCraft's MCP
bridge.
