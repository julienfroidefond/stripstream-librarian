#!/usr/bin/env bash
# Tests for `generate-changelog.sh`. Builds a throwaway repository with tags
# and conventional commits, then checks the rendered Markdown.
#
#   scripts/generate-changelog.test.sh
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
generator="$here/generate-changelog.sh"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

repo="$tmp/repo"
mkdir -p "$repo"
cd "$repo"
git init -q
git config user.email "test@example.com"
git config user.name "Test"

commit() { # <message> <date>
    echo "$1" >>log.txt
    git add log.txt
    GIT_AUTHOR_DATE="$2" GIT_COMMITTER_DATE="$2" git commit -q -m "$1"
}

commit "feat(core): first feature (#1)" "2026-01-01T00:00:00Z"
git tag v1.0.0
commit "fix(api): repair thing (#2)" "2026-01-02T00:00:00Z"
git tag v1.0.1
commit "chore: bump version to 1.0.2" "2026-01-03T00:00:00Z"
git tag v1.0.2
commit "docs: explain usage" "2026-01-04T00:00:00Z"
git tag v1.1.0
commit "feat: unreleased thing (#5)" "2026-01-05T00:00:00Z"

failures=0
expect_contains() {
    if ! grep -qF -- "$1" "$2"; then
        echo "FAIL: expected to find: $1" >&2
        failures=$((failures + 1))
    fi
}

expect_absent() {
    if grep -qF -- "$1" "$2"; then
        echo "FAIL: expected to be absent: $1" >&2
        failures=$((failures + 1))
    fi
}

"$generator" --repo "$repo" --out "$tmp/out.md" --all >/dev/null

expect_contains "## À venir" "$tmp/out.md"
expect_contains "unreleased thing ([#5](${CHANGELOG_REPO_URL:-https://github.com/julienfroidefond/stripstream-librarian}/pull/5))" "$tmp/out.md"
expect_contains "## v1.1.0 — 2026-01-04" "$tmp/out.md"
expect_contains "### Documentation" "$tmp/out.md"
expect_contains "## v1.0.1 — 2026-01-02" "$tmp/out.md"
expect_contains "**api** — repair thing" "$tmp/out.md"
expect_contains "## v1.0.0 — 2026-01-01" "$tmp/out.md"
expect_contains "**core** — first feature" "$tmp/out.md"
expect_absent "## v1.0.2" "$tmp/out.md"

"$generator" --repo "$repo" --out "$tmp/release.md" --all --release 1.2.0 >/dev/null
expect_contains "## v1.2.0 — " "$tmp/release.md"
expect_absent "## À venir" "$tmp/release.md"

"$generator" --repo "$repo" --out "$tmp/default.md" >/dev/null
expect_contains "## v1.0.0" "$tmp/default.md"
expect_absent "versions antérieures" "$tmp/default.md"

"$generator" --repo "$repo" --out "$tmp/limited.md" --limit 1 >/dev/null
expect_contains "## À venir" "$tmp/limited.md"
expect_contains "## v1.1.0" "$tmp/limited.md"
expect_absent "## v1.0.1" "$tmp/limited.md"
expect_contains "versions antérieures" "$tmp/limited.md"

cp "$tmp/out.md" "$tmp/committed.md"
"$generator" --repo "$repo" --out "$tmp/committed.md" --all --check >/dev/null
printf 'stale\n' >>"$tmp/committed.md"
if "$generator" --repo "$repo" --out "$tmp/committed.md" --all --check >/dev/null 2>&1; then
    echo "FAIL: --check accepted a stale file" >&2
    failures=$((failures + 1))
fi

if [ "$failures" -gt 0 ]; then
    echo "$failures failure(s)" >&2
    exit 1
fi
echo "changelog generator: OK"
