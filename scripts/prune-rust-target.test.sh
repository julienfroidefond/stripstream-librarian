#!/usr/bin/env bash
# Regression tests for scripts/prune-rust-target.sh.
set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
prune="$script_dir/prune-rust-target.sh"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

mkdir -p "$tmp/one/target/debug/incremental/crate-1" \
  "$tmp/one/target/debug/deps" \
  "$tmp/two/target/release/incremental/crate-2"
touch "$tmp/one/target/debug/incremental/crate-1/file" \
  "$tmp/one/target/debug/deps/libfoo.rlib" \
  "$tmp/two/target/release/incremental/crate-2/file"

# Dry run reports the caches but leaves them in place.
output=$("$prune" "$tmp/one" "$tmp/two")
grep -q "would remove" <<<"$output" || fail "dry run should report removals"
[[ -d "$tmp/one/target/debug/incremental" ]] || fail "dry run removed debug incremental"
[[ -d "$tmp/two/target/release/incremental" ]] || fail "dry run removed release incremental"

# Apply removes incremental caches but keeps compiled dependencies.
"$prune" --apply "$tmp/one" "$tmp/two" >/dev/null
[[ ! -e "$tmp/one/target/debug/incremental" ]] || fail "debug incremental was not removed"
[[ ! -e "$tmp/two/target/release/incremental" ]] || fail "release incremental was not removed"
[[ -f "$tmp/one/target/debug/deps/libfoo.rlib" ]] || fail "deps/ must be preserved"

# Applying again is a no-op.
"$prune" --apply "$tmp/one" "$tmp/two" >/dev/null

# Invalid input is rejected.
if "$prune" --stale-days nope "$tmp/one" >/dev/null 2>&1; then
  fail "non-numeric --stale-days accepted"
fi
if "$prune" --unknown >/dev/null 2>&1; then
  fail "unknown option accepted"
fi
"$prune" --help >/dev/null || fail "--help failed"

echo "ok"
