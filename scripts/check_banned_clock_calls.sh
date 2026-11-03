#!/usr/bin/env bash
# PRD 4.3 item 5 / PV-SEC-06: protocol clock must not use uptimeMillis, systemUptime,
# mach_absolute_time, Rust Instant or Dart Stopwatch; wall clock (SystemTime) is not a protocol clock either.
set -euo pipefail
cd "$(dirname "$0")/.."
status=0

# Native plugin sources and the Rust core: platform uptime APIs never allowed.
if grep -rnE 'uptimeMillis|systemUptime|mach_absolute_time|Stopwatch' core/src packages/meshpay_native \
  --include='*.rs' --include='*.kt' --include='*.java' --include='*.swift' --include='*.dart' \
  --exclude-dir=build --exclude-dir=.dart_tool --exclude-dir=target; then
  status=1
fi

# Rust core: Instant/SystemTime only in files that are not the protocol clock
# (dtn_buffer.rs: local rate limiter; api.rs: self-check timing and scratch filename).
if grep -rnE '\bInstant\b|SystemTime' core/src --include='*.rs' \
  | grep -vE '^core/src/(dtn_buffer|api|frb_generated)\.rs:'; then
  status=1
fi

[ "$status" -eq 0 ] || { echo "banned clock call found (PRD 4.3 item 5)" >&2; exit 1; }
