#!/bin/bash
# One repeat of the tuning benchmark (23 tasks in 4 sets) on one SolidCraft window, with the written Thinking report.
# usage: [KIND=hy|sc] [SETS="specs12:simple ..."] run_v.sh REP PORT MODEL_ID OUTDIR [SERVER.py]
#   REP: repeat number (1-3), PORT: SolidCraft control port, MODEL_ID: e.g. claude-haiku-5-5,
#   OUTDIR: runs go to OUTDIR/<set>-r<REP>/<task>-<kind>-<model>/, SERVER.py: scx server (default ../../scx.py)
PY=${PY:-python}
cd "$(dirname "$0")"
export THINK=${THINK-1} SC_PORT=$2
[ -n "$5" ] && export HY_SERVER="$5"
for set in ${SETS:-specs12:simple specsBig:big specsRel:rel specsEdit:edit}; do
  f=${set%%:*}; n=${set##*:}
  export SPECS=$f.json RUNS=$4/$n-r$1
  for part in $($PY -c "import json;print(' '.join(json.load(open('$f.json'))))"); do
    [ -f "$RUNS/$part-${KIND:-hy}-$3/result.json" ] && continue
    echo "== $n $part $(date +%T)"
    $PY -I bench.py ${KIND:-hy} $part $3
  done
done
echo "== done $(date +%T)"
