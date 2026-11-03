#!/usr/bin/env bash
# Refuse a release artifact unless (1) its tag is an annotated tag with a valid signature
# (GitHub API verification.verified) and (2) the artifact has a valid build-provenance
# attestation produced by this repository's release workflow for that tag.
# Needs the GitHub CLI (gh) logged in or GH_TOKEN set. Deploys nothing and needs no secrets.
#
# Usage:
#   verify_release.sh <tag> <artifact> [owner/repo]
#   verify_release.sh --self-test [owner/repo]
#
# Injected-failure cases (run by --self-test; both MUST be refused, otherwise exit 1):
#   1. Artifact without attestation: a random local file is verified against the real repo.
#   2. Tag without valid signature: a tag that does not exist (same API path as a lightweight
#      or unsigned tag, which fail the "annotated + verified" check).
# Exit: 0 = release accepted (or self-test passed), 1 = release refused (or self-test failed).
set -euo pipefail

default_repo="Barba-Tech-CO/MeshPay"

verify_tag() {
  local repo="$1" tag="$2" ref_type sha verified
  ref_type="$(gh api "repos/$repo/git/ref/tags/$tag" --jq .object.type 2>/dev/null)" || {
    printf 'REFUSED: tag %s not found in %s\n' "$tag" "$repo" >&2
    return 1
  }
  if [[ "$ref_type" != "tag" ]]; then
    printf 'REFUSED: tag %s is lightweight (not annotated), so it cannot be signed\n' "$tag" >&2
    return 1
  fi
  sha="$(gh api "repos/$repo/git/ref/tags/$tag" --jq .object.sha)"
  verified="$(gh api "repos/$repo/git/tags/$sha" --jq .verification.verified)"
  if [[ "$verified" != "true" ]]; then
    printf 'REFUSED: tag %s has no valid signature (verification.verified=%s)\n' "$tag" "$verified" >&2
    return 1
  fi
  printf 'OK: tag %s is annotated and signature-verified\n' "$tag"
}

verify_artifact() {
  local repo="$1" tag="$2" artifact="$3"
  if [[ ! -f "$artifact" ]]; then
    printf 'REFUSED: artifact not found: %s\n' "$artifact" >&2
    return 1
  fi
  if ! gh attestation verify "$artifact" --repo "$repo" \
    --signer-workflow "$repo/.github/workflows/release.yml" \
    --source-ref "refs/tags/$tag" --deny-self-hosted-runners >/dev/null 2>&1; then
    printf 'REFUSED: %s has no valid attestation from %s release workflow at %s\n' \
      "$artifact" "$repo" "$tag" >&2
    return 1
  fi
  printf 'OK: %s has a valid build-provenance attestation\n' "$artifact"
}

verify_release() {
  local repo="$1" tag="$2" artifact="$3" status=0
  verify_tag "$repo" "$tag" || status=1
  verify_artifact "$repo" "$tag" "$artifact" || status=1
  return "$status"
}

if [[ "${1:-}" == "--self-test" ]]; then
  repo="${2:-$default_repo}"
  scratch="$(mktemp -d)"
  trap 'rm -rf "$scratch"' EXIT
  printf 'not a release\n' > "$scratch/unattested.bin"
  failed=0
  if verify_artifact "$repo" "v0.0.0-selftest" "$scratch/unattested.bin"; then
    printf 'SELF-TEST FAIL: unattested artifact was accepted\n' >&2
    failed=1
  fi
  if verify_tag "$repo" "v0.0.0-selftest-unsigned"; then
    printf 'SELF-TEST FAIL: unsigned tag was accepted\n' >&2
    failed=1
  fi
  [[ "$failed" -eq 0 ]] && printf 'SELF-TEST OK: 2 of 2 injected failures refused\n'
  exit "$failed"
fi

if [[ $# -lt 2 || $# -gt 3 ]]; then
  sed -n '2,17p' "$0" >&2
  exit 1
fi
verify_release "${3:-$default_repo}" "$1" "$2"
