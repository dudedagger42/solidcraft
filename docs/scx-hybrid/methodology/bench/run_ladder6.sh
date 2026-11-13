#!/bin/bash
# L6 iterative: 3 tasks x 2 setups x 3 repeats, one repeat per SolidCraft window (ports 7879-7881);
# each run is one conversation of 4-5 change requests (bench_iter.py).
# usage: run_ladder6.sh [MODEL_ID] [SERVER.py]   (the published runs: claude-haiku-5-5 and scx v14)
PY=${PY:-python}
cd "$(dirname "$0")"
MODEL=${1:-claude-haiku-5-5}
[ -n "$2" ] && export HY_SERVER="$2"
for rep in 1 2 3; do
  port=$((7878 + rep))
  ( export THINK=${THINK-1} SC_PORT=$port SPECS=specsL6.json RUNS=runsLadder/L6-r$rep
    for kind in sc hy; do
      for t in L6_gearbox_iter L6_pulley_iter L6_motor_plate_iter; do
        [ -f "$RUNS/$t-$kind-$MODEL/result.json" ] && continue
        echo "== $t $kind $(date +%T)"
        $PY -I bench_iter.py $kind $t $MODEL
      done
    done ) > logLadder-L6-r$rep.txt 2>&1 &
done
wait
echo "== level 6 done $(date +%T)"
