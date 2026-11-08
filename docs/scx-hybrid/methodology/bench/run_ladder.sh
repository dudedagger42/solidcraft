#!/bin/bash
# One ladder level (L1-L5) = 3 tasks x 2 setups x 3 repeats = 18 runs, one repeat per SolidCraft window (ports 7879-7881).
# usage: run_ladder.sh LEVEL [MODEL_ID] [SERVER.py]   (the published runs: claude-haiku-5-5 and scx v14)
cd "$(dirname "$0")"
L=$1; MODEL=${2:-claude-haiku-5-5}; SERVER=${3:-}
for rep in 1 2 3; do
  port=$((7878 + rep))
  ( for kind in sc hy; do
      SETS="specsL$L:L$L" KIND=$kind bash run_v.sh $rep $port $MODEL runsLadder $SERVER
    done ) > logLadder-L$L-r$rep.txt 2>&1 &
done
wait
echo "== level $L done $(date +%T)"
