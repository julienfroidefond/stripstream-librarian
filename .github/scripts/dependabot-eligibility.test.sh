#!/usr/bin/env bash
# Tests for `dependabot-eligibility.jq`, the decision logic of
# `.github/workflows/dependabot-automerge.yml`. No dependency beyond jq:
#
#   .github/scripts/dependabot-eligibility.test.sh
#
# Run by the `Dependabot policy` job of `ci.yml`.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
program="$here/dependabot-eligibility.jq"
failures=0

old=$(printf 'a%.0s' {1..64})
new=$(printf 'b%.0s' {1..64})

# deps <dep>... — one JSON array of `updated-dependencies-json` entries.
deps() {
  local IFS=,
  printf '[%s]' "$*"
}

# dep <ecosystem> <updateType> <prevVersion> <dependencyGroup>
dep() {
  printf '{"packageEcosystem":"%s","updateType":"%s","prevVersion":"%s","dependencyGroup":"%s"}' \
    "$1" "$2" "$3" "$4"
}

# files <patch>... — one PR file per patch, as returned by the pulls/files API.
files() {
  local patch_json
  for patch_json in "$@"; do
    jq -nc --arg p "$patch_json" '{filename:"Dockerfile", patch:$p}'
  done | jq -sc .
}

# check <name> <deps-json> <files-json> <eligible> <reason-substring>
check() {
  local name=$1 deps_json=$2 files_json=$3 want=$4 reason=$5 out eligible decision
  out=$(jq -n --argjson deps "$deps_json" --argjson files "$files_json" -f "$program")
  eligible=$(jq -r .eligible <<<"$out")
  decision=$(jq -r .reason <<<"$out")
  if [ "$eligible" != "$want" ] || [[ "$decision" != *"$reason"* ]]; then
    printf 'FAIL %s\n  expected eligible=%s reason~%s\n  got      %s\n' \
      "$name" "$want" "$reason" "$out"
    failures=$((failures + 1))
  else
    printf 'ok   %-42s %s\n' "$name" "$decision"
  fi
}

# ---------------------------------------------------------------- semver rules
check "patch" \
  "$(deps "$(dep cargo version-update:semver-patch 1.2.3 '')")" '[]' \
  true "patch/minor"

check "minor post-1.0" \
  "$(deps "$(dep cargo version-update:semver-minor 1.54.0 '')")" '[]' \
  true "patch/minor"

check "minor 0.x is breaking" \
  "$(deps "$(dep cargo version-update:semver-minor 0.27.0 '')")" '[]' \
  false "0.x"

check "patch 0.x is fine" \
  "$(deps "$(dep npm version-update:semver-patch 0.4.1 '')")" '[]' \
  true "patch/minor"

check "major" \
  "$(deps "$(dep cargo version-update:semver-major 1.2.3 '')")" '[]' \
  false "major"

check "grouped minor (no prevVersion)" \
  "$(deps "$(dep cargo version-update:semver-minor '' rust-non-major)")" '[]' \
  true "patch/minor"

check "grouped PR with one 0.x bump" \
  "$(deps "$(dep cargo version-update:semver-patch 1.0.1 '')" \
          "$(dep cargo version-update:semver-minor 0.9.0 '')")" '[]' \
  false "0.x"

check "empty updateType outside docker" \
  "$(deps "$(dep cargo '' '' '')")" '[]' \
  false "unrecognised"

# ------------------------------------------------------------- digest bumps
from_line=$(printf 'FROM lukemathwalker/cargo-chef:latest-rust-1-bookworm@sha256:%s' "$old")
to_line=$(printf 'FROM lukemathwalker/cargo-chef:latest-rust-1-bookworm@sha256:%s' "$new")

check "docker digest bump" \
  "$(deps "$(dep docker '' '' '')")" \
  "$(files "$(printf -- '-%s\n+%s' "$from_line" "$to_line")")" \
  true "digest-only"

check "docker-compose digest bump" \
  "$(deps "$(dep docker-compose '' '' '')")" \
  "$(files "$(printf -- '-  image: postgres:16-alpine@sha256:%s\n+  image: postgres:16-alpine@sha256:%s' "$old" "$new")")" \
  true "digest-only"

check "digest bump in a multi-stage Dockerfile" \
  "$(deps "$(dep docker '' '' '')")" \
  "$(files "$(printf -- '-%s AS builder\n+%s AS builder' "$from_line" "$to_line")")" \
  true "digest-only"

check "digest bump without the diff (files not fetched)" \
  "$(deps "$(dep docker '' '' '')")" '[]' \
  false "digest"

check "non-semver tag change is not a digest bump" \
  "$(deps "$(dep docker '' '' '')")" \
  "$(files "$(printf -- '-%s\n+%s' \
    "$from_line" \
    "$(printf 'FROM lukemathwalker/cargo-chef:latest-rust-1-trixie@sha256:%s' "$new")")")" \
  false "digest"

check "digest bump mixed with another change" \
  "$(deps "$(dep docker '' '' '')")" \
  "$(files "$(printf -- '-%s\n+%s\n+RUN echo hello' "$from_line" "$to_line")")" \
  false "digest"

check "two images, only one digest moved" \
  "$(deps "$(dep docker '' '' '')")" \
  "$(files "$(printf -- '-%s\n+%s\n-FROM rust:1@sha256:%s\n+FROM rust:1.99@sha256:%s' \
    "$from_line" "$to_line" "$old" "$new")")" \
  false "digest"

if [ "$failures" -ne 0 ]; then
  printf '\n%d test(s) failed\n' "$failures"
  exit 1
fi
printf '\nall tests passed\n'
