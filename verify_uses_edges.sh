#!/usr/bin/env bash
set -e

echo "=== Step 1: cargo check ==="
cargo check --features backend

echo ""
echo "=== Step 2: Run Uses edge tests ==="
cargo test --features backend golang::extractor_test::test_uses_edge

echo ""
echo "=== Step 3: Run all backend tests ==="
cargo test --features backend

echo ""
echo "=== Step 4: cargo clippy ==="
cargo clippy --features backend -- -D warnings

echo ""
echo "=== ALL CHECKS PASSED ==="
