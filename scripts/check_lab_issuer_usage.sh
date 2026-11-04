#!/usr/bin/env bash
# The lab issuer root is public: only the lab helper and the issuer config
# helper may reference it in app/lib (release must never pin it).
set -euo pipefail
cd "$(dirname "$0")/.."
if grep -rnE 'labIssuer|LabChequeStubHelper\.issuer' app/lib --include='*.dart' \
  | grep -vE '^app/lib/core/tx/(lab_cheque_stub_helper|issuer_config_helper)\.dart:'; then
  echo "lab issuer referenced outside lab/config helpers" >&2
  exit 1
fi
