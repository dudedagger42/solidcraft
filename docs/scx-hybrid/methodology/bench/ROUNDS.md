# Bench rounds (Haiku)

Pass = every expected solid within 0.1% volume and 0.05 mm bbox, and no extra solids. Tokens = input tokens incl. cache, thousands. One run per part per round, so ±1 pass is noise.

## simple: 12 specs with explicit coordinates (10 single parts, 2 four-part assemblies)

| round | setup | pass | exact bodies | tokens k | tokens k per pass | tool calls | minutes | $ |
|---|---|---|---|---|---|---|---|---|
| FC | FreeCAD MCP (neka-nat) | 8/12 | 14/18 | 5885 | 736 | 191 | 24.0 | 1.63 |
| SC | SolidCraft MCP (current, 21 tools) | 11/12 | 16/18 | 6130 | 557 | 159 | 13.5 | 1.66 |
| H1 | hybrid v1 | 8/12 | 14/18 | 2873 | 359 | 81 | 12.6 | 1.11 |
| H2 | hybrid v2 | 11/12 | 17/18 | 940 | 85 | 23 | 9.5 | 0.8 |
| H3 | hybrid v3 | 11/12 | 17/18 | 1569 | 143 | 39 | 10.1 | 0.87 |
| H4 | hybrid v4 | 9/12 | 13/18 | 981 | 109 | 25 | 7.6 | 0.67 |
| H5 | hybrid v5 | 12/12 | 18/18 | 819 | 68 | 19 | 7.5 | 0.67 |
| H5b | hybrid v5 (repeat) | 10/12 | 16/18 | 939 | 94 | 23 | 9.8 | 0.8 |
| H6 | hybrid v6 | 12/12 | 18/18 | 1009 | 84 | 26 | 7.5 | 0.69 |
| H7 | hybrid v7 | 12/12 | 18/18 | 933 | 78 | 24 | 6.8 | 0.64 |
| H7b | hybrid v7 (repeat) | 12/12 | 18/18 | 1034 | 86 | 27 | 8.2 | 0.73 |

## big: 4 larger specs: 16-fin heatsink, 16-part standoffs, 8-part rack, 11-part bolted flange joint

| round | setup | pass | exact bodies | tokens k | tokens k per pass | tool calls | minutes | $ |
|---|---|---|---|---|---|---|---|---|
| FC | FreeCAD MCP (neka-nat) | 2/4 | 20/36 | 635 | 317 | 20 | 3.0 | 0.23 |
| SC | SolidCraft MCP (current, 21 tools) | 4/4 | 36/36 | 2205 | 551 | 56 | 6.0 | 0.72 |
| H2 | hybrid v2 | 4/4 | 36/36 | 315 | 79 | 8 | 2.6 | 0.23 |
| H3 | hybrid v3 | 3/4 | 33/36 | 381 | 127 | 10 | 3.2 | 0.28 |
| H4 | hybrid v4 | 4/4 | 36/36 | 240 | 60 | 5 | 3.1 | 0.26 |
| H5 | hybrid v5 | 4/4 | 36/36 | 344 | 86 | 9 | 2.5 | 0.22 |
| H6 | hybrid v6 | 4/4 | 36/36 | 374 | 94 | 10 | 2.6 | 0.23 |
| H7 | hybrid v7 | 3/4 | 36/36 | 549 | 183 | 16 | 2.9 | 0.26 |

## rel: 4 specs phrased like a person would (centred, evenly spaced, stacked): bushing plate, slotted cube, pipe tee, 6-part washer stack

| round | setup | pass | exact bodies | tokens k | tokens k per pass | tool calls | minutes | $ |
|---|---|---|---|---|---|---|---|---|
| FC | FreeCAD MCP (neka-nat) | 1/4 | 7/9 | 950 | 950 | 35 | 4.0 | 0.28 |
| SC | SolidCraft MCP (current, 21 tools) | 4/4 | 9/9 | 1764 | 441 | 43 | 5.2 | 0.57 |
| H2 | hybrid v2 | 4/4 | 9/9 | 252 | 63 | 6 | 1.7 | 0.17 |
| H3 | hybrid v3 | 4/4 | 9/9 | 284 | 71 | 7 | 2.4 | 0.21 |
| H4 | hybrid v4 | 4/4 | 9/9 | 291 | 73 | 7 | 2.6 | 0.22 |
| H5 | hybrid v5 | 4/4 | 9/9 | 447 | 112 | 11 | 4.1 | 0.33 |
| H6 | hybrid v6 | 4/4 | 9/9 | 609 | 152 | 16 | 3.3 | 0.29 |
| H7 | hybrid v7 | 3/4 | 3/9 | 266 | 89 | 6 | 3.8 | 0.28 |

## edit: 3 edits to an existing 24-part assembly (rack + board on 15 standoffs): move parts, lengthen posts + add a shelf, drill shelves + delete 2 standoffs. FreeCAD not run (no preload there)

| round | setup | pass | exact bodies | tokens k | tokens k per pass | tool calls | minutes | $ |
|---|---|---|---|---|---|---|---|---|
| SC | SolidCraft MCP (current, 21 tools) | 2/3 | 55/71 | 1629 | 815 | 34 | 5.0 | 0.58 |
| H3 | hybrid v3 | 3/3 | 71/71 | 675 | 225 | 21 | 1.9 | 0.21 |
| H4 | hybrid v4 | 3/3 | 71/71 | 662 | 221 | 21 | 2.0 | 0.21 |
| H5 | hybrid v5 | 3/3 | 71/71 | 380 | 127 | 12 | 1.5 | 0.15 |
| H6 | hybrid v6 | 3/3 | 71/71 | 622 | 207 | 20 | 1.7 | 0.19 |
| H7 | hybrid v7 | 2/3 | 70/71 | 1089 | 544 | 31 | 2.9 | 0.29 |

## Per part (pass, input tokens k / tool calls)

| part | FC | SC | H1 | H2 | H3 | H4 | H5 | H5b | H6 | H7 | H7b |
|---|---|---|---|---|---|---|---|---|---|---|---|
| plate | ✅ 227k/10 | ✅ 205k/6 | ✅ 74k/2 | ✅ 73k/2 | ✅ 48k/1 | ✅ 73k/2 | ✅ 73k/2 | ✅ 75k/2 | ✅ 74k/2 | ✅ 73k/2 | ✅ 73k/2 |
| bracket | ❌ 608k/14 | ✅ 1069k/24 | ❌ 54k/1 | ✅ 82k/2 | ✅ 86k/2 | ✅ 78k/2 | ✅ 57k/1 | ✅ 58k/1 | ✅ 80k/2 | ✅ 109k/3 | ✅ 56k/1 |
| enclosure | ❌ 288k/9 | ✅ 591k/17 | ❌ 89k/2 | ❌ 84k/2 | ❌ 87k/2 | ✅ 93k/2 | ✅ 93k/2 | ✅ 82k/2 | ✅ 93k/2 | ✅ 110k/3 | ✅ 91k/2 |
| flange | ✅ 252k/8 | ✅ 555k/15 | ❌ 564k/19 | ✅ 59k/1 | ✅ 79k/2 | ✅ 83k/2 | ✅ 78k/2 | ✅ 83k/2 | ✅ 118k/3 | ✅ 75k/2 | ✅ 153k/5 |
| shaft | ❌ 399k/12 | ✅ 484k/15 | ❌ 891k/24 | ✅ 80k/2 | ✅ 154k/5 | ❌ 54k/1 | ✅ 51k/1 | ✅ 84k/2 | ✅ 154k/5 | ✅ 54k/1 | ✅ 82k/2 |
| spacer | ✅ 135k/4 | ✅ 570k/14 | ✅ 94k/2 | ✅ 94k/2 | ✅ 95k/2 | ❌ 78k/2 | ✅ 86k/2 | ❌ 111k/3 | ✅ 83k/2 | ✅ 55k/1 | ✅ 111k/3 |
| channel | ✅ 275k/19 | ✅ 243k/7 | ✅ 53k/1 | ✅ 85k/2 | ✅ 82k/2 | ✅ 51k/1 | ✅ 50k/1 | ❌ 89k/2 | ✅ 81k/2 | ✅ 84k/2 | ✅ 56k/1 |
| rounded | ❌ 2190k/46 | ✅ 114k/3 | ✅ 75k/2 | ✅ 73k/2 | ✅ 73k/2 | ✅ 98k/3 | ✅ 75k/2 | ✅ 77k/2 | ✅ 74k/2 | ✅ 75k/2 | ✅ 127k/4 |
| frustum | ✅ 158k/5 | ✅ 392k/10 | ✅ 743k/22 | ✅ 98k/3 | ✅ 72k/2 | ✅ 73k/2 | ✅ 48k/1 | ✅ 74k/2 | ✅ 73k/2 | ✅ 72k/2 | ✅ 48k/1 |
| rail | ✅ 1055k/55 | ✅ 264k/7 | ✅ 73k/2 | ✅ 73k/2 | ✅ 74k/2 | ✅ 48k/1 | ✅ 48k/1 | ✅ 74k/2 | ✅ 48k/1 | ✅ 74k/2 | ✅ 73k/2 |
| clamp | ✅ 166k/5 | ❌ 1056k/25 | ✅ 81k/2 | ✅ 87k/2 | ✅ 54k/1 | ✅ 78k/2 | ✅ 79k/2 | ✅ 80k/2 | ✅ 78k/2 | ✅ 74k/2 | ✅ 80k/2 |
| bearing | ✅ 132k/4 | ✅ 588k/16 | ✅ 83k/2 | ✅ 53k/1 | ✅ 664k/16 | ❌ 175k/5 | ✅ 81k/2 | ✅ 54k/1 | ✅ 53k/1 | ✅ 77k/2 | ✅ 84k/2 |
| heatsink | ✅ 244k/8 | ✅ 384k/11 | — | ✅ 73k/2 | ✅ 159k/5 | ✅ 49k/1 | ✅ 48k/1 | — | ✅ 73k/2 | ✅ 74k/2 | — |
| standoffs | ❌ 132k/4 | ✅ 470k/10 | — | ✅ 83k/2 | ✅ 87k/2 | ✅ 54k/1 | ✅ 84k/2 | — | ✅ 83k/2 | ❌ 110k/3 | — |
| rack | ✅ 128k/4 | ✅ 546k/15 | — | ✅ 81k/2 | ✅ 81k/2 | ✅ 79k/2 | ✅ 80k/2 | — | ✅ 106k/3 | ✅ 80k/2 | — |
| flangepair | ❌ 132k/4 | ✅ 804k/20 | — | ✅ 78k/2 | ❌ 54k/1 | ✅ 59k/1 | ✅ 131k/4 | — | ✅ 112k/3 | ✅ 285k/9 | — |
| bushplate | ❌ 278k/9 | ✅ 994k/24 | — | ✅ 75k/2 | ✅ 103k/3 | ✅ 79k/2 | ✅ 79k/2 | — | ✅ 101k/3 | ✅ 74k/2 | — |
| slotcube | ❌ 267k/10 | ✅ 142k/4 | — | ✅ 48k/1 | ✅ 49k/1 | ✅ 50k/1 | ✅ 75k/2 | — | ✅ 48k/1 | ✅ 49k/1 | — |
| tee | ❌ 159k/5 | ✅ 424k/9 | — | ✅ 78k/2 | ✅ 55k/1 | ✅ 84k/2 | ✅ 209k/5 | — | ✅ 355k/9 | ✅ 64k/1 | — |
| washers | ✅ 247k/11 | ✅ 204k/6 | — | ✅ 49k/1 | ✅ 77k/2 | ✅ 78k/2 | ✅ 85k/2 | — | ✅ 105k/3 | ❌ 79k/2 | — |
| edit_move | — | ❌ 876k/17 | — | — | ✅ 98k/3 | ✅ 100k/3 | ✅ 99k/3 | — | ✅ 99k/3 | ✅ 99k/3 | — |
| edit_add | — | ✅ 333k/7 | — | — | ✅ 422k/13 | ✅ 461k/15 | ✅ 179k/6 | — | ✅ 370k/12 | ❌ 865k/24 | — |
| edit_holes | — | ✅ 420k/10 | — | — | ✅ 154k/5 | ✅ 101k/3 | ✅ 102k/3 | — | ✅ 153k/5 | ✅ 125k/4 | — |

## What changed each round

- **FC** (FreeCAD MCP (neka-nat)): baseline: model writes FreeCAD Python via execute_code
- **SC** (SolidCraft MCP (current, 21 tools)): baseline: execute/batch with command ids
- **H1** (hybrid v1): first hybrid: Python scripts in SolidCraft vocabulary, auto diff after each run
- **H2** (hybrid v2): fix: script accepted under code/script/command (was silently ignored: flange+shaft); slot(center, overall length, width); instructions point to shell() for hollowing; 'one script per part' nudge
- **H3** (hybrid v3): remove(bodies) helper; bare last expression is printed (measure('X')); alike new bodies grouped in one summary line, long 'gone' lists compressed; shell wording: 'a cut box leaves square inner corners'; 'use Python loops for patterns and repeated parts'
- **H4** (hybrid v4): hollow(t, open='top'...) helper named after the word people use; UTF-8 stdin/stdout (non-ASCII text was garbled); `design` tool gives a compact parts outline with identical parts grouped instead of raw component JSON
- **H5** (hybrid v5): join/cut/intersect default to the last body made or changed (a cut box no longer slices every body it touches, e.g. the posts); feature objects accepted where a body name goes; warning when name= is already taken; center=True/False accepted; cmd('combine') hint says call combine() directly
- **H5b** (hybrid v5 (repeat)): same v5 server run again on the simple set, to show run-to-run noise
- **H6** (hybrid v6): cylinder/cone accept start=, end= (centres of the end faces) or center=, so 'from x=-20 to x=100' maps straight in (H4 bearing put `at` mid-length); short 'words -> calls' table (disc/rod -> cylinder, ring/tube/washer -> two circles, hexagon -> polygon, hollow -> hollow(), blind hole -> depth=) after H3 built discs as boxes
- **H7** (hybrid v7): from v6's costliest runs (tee 9 calls, edit_add 12): remove() skips names that do not exist (note instead of stopping the script) and prints nothing itself; NameError on a body name says names are strings and variables do not survive between runs; hole() accepts body=/into=; a body removed and rebuilt under the same name is reported ('= Shelf5 rebuilt ... same as before'), so a notch that did not happen shows; words table: notch/cut-out -> extrude outline then box(op='cut'). RESULT: 20/23 (washers, standoffs, edit_add misses were Haiku modelling slips; edit_add hit a kernel error from a tangent corner hole, 24 calls). Second regression in a row -> tuning stopped; v5 stays the best config.
- **H7b** (hybrid v7 (repeat)): same v7 server run again on the simple set, for noise
