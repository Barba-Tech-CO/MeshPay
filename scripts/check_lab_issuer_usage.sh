#!/usr/bin/env bash
# The lab issuer root is public: only exact lab/config references are allowed
# in app/lib (release must never pin it).
set -euo pipefail
cd "$(dirname "$0")/.."
if grep -rnE 'labIssuer|LabChequeStubHelper\.issuer' app/lib --include='*.dart' \
  | grep -vE '^app/lib/core/tx/(lab_cheque_stub_helper|issuer_config_helper)\.dart:'; then
  echo "lab issuer referenced outside lab/config helpers" >&2
  exit 1
fi
if grep -rnE 'labSnapshotCert|LabDtnSnapshotCertHelper' app/lib --include='*.dart' \
  | grep -vE '^app/lib/core/dtn/lab_dtn_snapshot_cert_helper\.dart:' \
  | grep -vE '^app/lib/core/dtn/snapshot_config_helper\.dart:[0-9]+:(import '\''lab_dtn_snapshot_cert_helper\.dart'\'';|    if \(!isRelease\) return LabDtnSnapshotCertHelper\.cert\(\);)$'; then
  echo "lab snapshot cert referenced outside lab/config helpers" >&2
  exit 1
fi
