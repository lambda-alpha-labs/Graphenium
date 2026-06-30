#!/usr/bin/env bash
set -euo pipefail

echo "=== Verifying Compiler Warnings ==="
cargo build --release 2> build.log
if grep -q "warning" build.log; then
    echo "FAIL: Warnings found in build log"
    exit 1
fi
echo "PASS: Warnings clean"

echo "=== Verifying Lazy Server Startup ==="
rm -f graphenium-out/graph.json
gm serve --graph graphenium-out/graph.json &
SERVER_PID=$!
sleep 1
if ! kill -0 $SERVER_PID 2>/dev/null; then
    echo "FAIL: Server exited when graph.json was missing"
    kill $SERVER_PID 2>/dev/null || true
    exit 1
fi
kill $SERVER_PID 2>/dev/null || true
echo "PASS: Server lazy startup working"

echo "=== Verifying Snapshot Typo ==="
if grep -rq "graphemium-snapshots" src/; then
    echo "FAIL: Found misspelling 'graphemium-snapshots' in source code"
    exit 1
fi
echo "PASS: No snapshot spelling typos found"

echo ""
echo "All robustness checks passed."
