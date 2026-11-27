#!/usr/bin/env bash
# Runs the federation load test (agency#529) in a release build and prints one table row per configuration.
# The numbers depend on the machine: record it next to them in docs/scale.md.
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo test --release --locked -p agency-node --test federation_load -- --ignored --nocapture --test-threads 1 2>&1 \
  | grep '^|'
