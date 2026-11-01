#!/usr/bin/env bash
set -euo pipefail

export PATH="$HOME/.cargo/bin:$PATH"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
temp_root="$(mktemp -d "${TMPDIR:-/tmp}/meshpay-security-ci.XXXXXX")"
expected_failures=0

cleanup() {
  rm -rf "$temp_root"
}
trap cleanup EXIT

for tool in cargo cargo-audit cargo-deny osv-scanner gitleaks git python3; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'Required tool is missing: %s\n' "$tool" >&2
    exit 1
  fi
done

copy_fixtures() {
  local destination="$1"
  mkdir -p "$destination/core" "$destination/app/design_system" "$destination/app/android/app"
  cp "$repo_root/core/Cargo.toml" "$repo_root/core/Cargo.lock" "$destination/core/"
  cp -R "$repo_root/core/src" "$destination/core/src"
  cp "$repo_root/app/pubspec.lock" "$destination/app/"
  if [[ -f "$repo_root/app/design_system/pubspec.lock" ]]; then
    cp "$repo_root/app/design_system/pubspec.lock" "$destination/app/design_system/"
  fi
  cp "$repo_root/app/android/app/gradle.lockfile" "$destination/app/android/app/"
  cp "$repo_root/deny.toml" "$destination/"
  cp "$repo_root/.gitleaks.toml" "$destination/"
  cp "$repo_root/osv-scanner.toml" "$destination/"
}

expect_failure() {
  local case_name="$1"
  local expected_text="$2"
  local log_file="$3"
  shift 3

  if "$@" >"$log_file" 2>&1; then
    printf '%s unexpectedly passed\n' "$case_name" >&2
    exit 1
  fi
  if ! grep -Fq "$expected_text" "$log_file"; then
    printf '%s failed for an unexpected reason\n' "$case_name" >&2
    sed -n '1,30p' "$log_file" >&2
    exit 1
  fi
  printf '%s failed as expected (%s)\n' "$case_name" "$expected_text"
}

run_osv() {
  local directory="$1"
  (
    cd "$directory"
    osv-scanner scan source \
      --config ./osv-scanner.toml \
      --lockfile app/pubspec.lock \
      --lockfile app/design_system/pubspec.lock \
      --lockfile app/android/app/gradle.lockfile
  )
}

run_osv_lockfile() {
  local directory="$1"
  local lockfile="$2"
  (
    cd "$directory"
    osv-scanner scan source --config ./osv-scanner.toml --lockfile "$lockfile"
  )
}

clean_status=0
run_clean_check() {
  local check_name="$1"
  local log_file="$temp_root/clean-${check_name}.log"
  local status
  shift

  if "$@" >"$log_file" 2>&1; then
    printf 'Clean check passed: %s\n' "$check_name"
  else
    status=$?
    clean_status=1
    printf 'Clean check failed: %s (exit %s)\n' "$check_name" "$status" >&2
    sed -n '1,18p' "$log_file" >&2
  fi
}

rust_case="$temp_root/case-rust-advisory"
copy_fixtures "$rust_case"
python3 - "$rust_case/core/Cargo.toml" <<'PY'
import sys

path = sys.argv[1]
manifest = open(path, encoding="utf-8").read()
manifest = manifest.replace(
    'tempfile = "3.27.0"',
    'tempfile = "3.27.0"\ntime = "=0.1.45"',
    1,
)
if 'time = "=0.1.45"' not in manifest:
    raise SystemExit("Could not add the RustSec fixture dependency")
open(path, "w", encoding="utf-8").write(manifest)
PY
(
  cd "$rust_case/core"
  cargo update
)
expect_failure \
  "cargo audit RustSec fixture" \
  "RUSTSEC-2020-0071" \
  "$temp_root/rust-audit.log" \
  cargo audit --file "$rust_case/core/Cargo.lock" --json
expect_failure \
  "cargo-deny RustSec fixture" \
  "RUSTSEC-2020-0071" \
  "$temp_root/rust-deny.log" \
  cargo deny --manifest-path "$rust_case/core/Cargo.toml" \
    --config "$rust_case/deny.toml" --locked check advisories
printf 'RustSec fixture: time 0.1.45, confirmed at runtime from the RustSec advisory database.\n'
expected_failures=$((expected_failures + 1))

license_case="$temp_root/case-license"
copy_fixtures "$license_case"
python3 - "$license_case/deny.toml" <<'PY'
import sys

path = sys.argv[1]
lines = open(path, encoding="utf-8").read().splitlines()
inside_licenses = False
inside_allow = False
updated = []
for line in lines:
    if line == "[licenses]":
        inside_licenses = True
    elif line.startswith("[") and line != "[licenses]":
        inside_licenses = False
    if inside_licenses and line.strip() == "allow = [":
        updated.append('allow = ["MIT"]')
        inside_allow = True
        continue
    if inside_allow:
        if line.strip() == "]":
            inside_allow = False
        continue
    updated.append(line)
if not any(line == 'allow = ["MIT"]' for line in updated):
    raise SystemExit("Could not restrict the license fixture allow list")
open(path, "w", encoding="utf-8").write("\n".join(updated) + "\n")
PY
expect_failure \
  "cargo-deny disallowed-license fixture" \
  "Apache-2.0" \
  "$temp_root/license.log" \
  cargo deny --manifest-path "$license_case/core/Cargo.toml" \
    --config "$license_case/deny.toml" --locked check licenses
expected_failures=$((expected_failures + 1))

source_case="$temp_root/case-registry"
copy_fixtures "$source_case"
python3 - "$source_case/core/Cargo.toml" <<'PY'
import sys

path = sys.argv[1]
manifest = open(path, encoding="utf-8").read()
manifest = manifest.replace(
    'tempfile = "3.27.0"',
    'tempfile = "3.27.0"\n'
    'libc-fixture = { package = "libc", git = "https://github.com/rust-lang/libc", '
    'rev = "9277f044b25dc46716caefc27e782de130874ad8" }',
    1,
)
if "libc-fixture" not in manifest:
    raise SystemExit("Could not add the foreign Git source fixture")
open(path, "w", encoding="utf-8").write(manifest)
PY
(
  cd "$source_case/core"
  cargo update
)
expect_failure \
  "cargo-deny foreign Git source fixture" \
  "source not explicitly allowed" \
  "$temp_root/registry.log" \
  cargo deny --manifest-path "$source_case/core/Cargo.toml" \
    --config "$source_case/deny.toml" --locked check sources
expected_failures=$((expected_failures + 1))

pub_case="$temp_root/case-pub-advisory"
copy_fixtures "$pub_case"
python3 - "$pub_case/app/pubspec.lock" <<'PY'
import sys

path = sys.argv[1]
lockfile = open(path, encoding="utf-8").read()
fixture = '''  archive:
    dependency: transitive
    description:
      name: archive
      sha256: "0c8368c9b3f0abbc193b9d6133649a614204b528982bebc7026372d61677ce3a"
      url: "https://pub.dev"
    source: hosted
    version: "3.3.7"
'''
if "\n  archive:\n" in lockfile:
    raise SystemExit("The Pub lockfile already contains the archive fixture")
if "sdks:\n" not in lockfile:
    raise SystemExit("Could not find the Pub lockfile insertion point")
open(path, "w", encoding="utf-8").write(lockfile.replace("sdks:\n", fixture + "sdks:\n", 1))
PY
expect_failure \
  "OSV Pub advisory fixture" \
  "GHSA-9v85-q87q-g4vg" \
  "$temp_root/pub-osv.log" \
  run_osv_lockfile "$pub_case" app/pubspec.lock
expected_failures=$((expected_failures + 1))

gradle_case="$temp_root/case-gradle-advisory"
copy_fixtures "$gradle_case"
printf '\norg.apache.logging.log4j:log4j-core:2.14.1=compileClasspath\n' \
  >>"$gradle_case/app/android/app/gradle.lockfile"
expect_failure \
  "OSV Gradle advisory fixture" \
  "GHSA-jfh8-c2jp-5v3q" \
  "$temp_root/gradle-osv.log" \
  run_osv_lockfile "$gradle_case" app/android/app/gradle.lockfile
expected_failures=$((expected_failures + 1))

secret_case="$temp_root/case-secret"
copy_fixtures "$secret_case"
mkdir -p "$secret_case/src"
python3 - "$secret_case/src/ci-smoke-secret.txt" <<'PY'
import sys

token = "ghp_" + "A1b2C3d4" + "E5f6G7h8" + "I9j0K1l2" + "M3n4O5p6" + "Q7r8S9t0"
with open(sys.argv[1], "w", encoding="utf-8") as file:
    file.write('fake_test_token = "' + token + '"\n')
PY
expect_failure \
  "Gitleaks secret fixture outside tests" \
  "leaks found" \
  "$temp_root/gitleaks.log" \
  gitleaks dir --config "$secret_case/.gitleaks.toml" --redact --no-banner "$secret_case"
expected_failures=$((expected_failures + 1))

printf '%s of 6 injected cases failed as expected\n' "$expected_failures"
if [[ "$expected_failures" -ne 6 ]]; then
  exit 1
fi

clean_case="$temp_root/clean"
copy_fixtures "$clean_case"
run_clean_check "cargo-audit" cargo audit --file "$clean_case/core/Cargo.lock"
for check in advisories licenses sources bans; do
  run_clean_check "cargo-deny-$check" \
    cargo deny --manifest-path "$clean_case/core/Cargo.toml" \
      --config "$clean_case/deny.toml" --locked check "$check"
done
run_clean_check "osv-lockfiles" run_osv "$clean_case"
run_clean_check "gitleaks-files" \
  gitleaks dir --config "$clean_case/.gitleaks.toml" --redact --no-banner "$clean_case"

if git -C "$repo_root" rev-parse --verify HEAD >/dev/null 2>&1; then
  printf 'Gitleaks history note: source repository has commits; CI scans full history.\n'
else
  printf 'Gitleaks history note: source repository has no commits; clean files were scanned in directory mode.\n'
fi

if [[ "$clean_status" -ne 0 ]]; then
  printf 'Clean-tree security checks failed.\n' >&2
  exit 1
fi

printf 'Clean-tree security checks passed.\n'
