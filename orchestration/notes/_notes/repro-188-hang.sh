#!/bin/bash
# Repeats the test that hung on spec/188's Windows shard 3 (run 37849242314),
# each run limited to 120 s, with CI's Windows test opt-ins.
# Usage: repro-188-hang.sh [filter] [runs]
cd /c/Users/ADMIN/Desktop/nguyenvu/pane-wt/spec-188
export CARGO_TARGET_DIR='C:\Users\ADMIN\Desktop\nguyenvu\pane-wt\_target'
export PANE_TEST_PROCESS_TREES=1 PANE_TEST_SYSTEM_ICONS=1
out=$(cargo test --locked -p pane-core --test integration --no-run 2>&1)
echo "$out" | tail -2
bin=$(echo "$out" | grep -o 'Executable .*' | sed -E 's/.*[(](.*)[)]/\1/' | tail -1)
echo "binary $bin"
filter=${1:-file_actions::}
for i in $(seq 1 "${2:-20}"); do
  start=$(date +%s)
  timeout 120 "$bin" "$filter" --test-threads=8 > "/tmp/repro-$i.log" 2>&1
  code=$?
  echo "run $i: exit $code in $(( $(date +%s) - start ))s $(grep -E '^test result' "/tmp/repro-$i.log")"
  if [ $code -eq 124 ]; then
    echo "HUNG on run $i"
    grep -E ' \.\.\. ' "/tmp/repro-$i.log" | grep -v ' ok$' | head
  fi
done
