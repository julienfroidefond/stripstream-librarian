#!/usr/bin/env bash
# Generate the documentation changelog from git tags and conventional commits.
#
#   scripts/generate-changelog.sh [--release VERSION] [--out PATH]
#                                 [--limit N | --all] [--check] [--repo DIR]
#
# Releases are the `v*` tags, newest first. Commits are grouped by
# conventional-commit type (feat → Nouveautés, fix → Corrections, …), the
# `chore: bump version to X` commits are dropped, and `(#123)` references are
# linked to their pull request.
#
# Run by the deploy workflow with `--release <new version>` before committing
# the version bump, so the docs image always ships an up-to-date changelog.
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
repo="$repo_root"
out="apps/docs/src/content/docs/reference/changelog.md"
release=""
limit=50
all=false
check=false

usage() {
    cat <<'EOF'
Usage: scripts/generate-changelog.sh [options]

  --release VERSION  Label the commits since the last tag as this version
  --out PATH         Output file (default: apps/docs/src/content/docs/reference/changelog.md)
  --limit N          Number of tagged releases to render (default: 50)
  --all              Render every tagged release
  --check            Exit 1 when the output differs from the committed file
  --repo DIR         Repository to read (default: this repository)
  -h, --help         Show this help
EOF
}

while (($#)); do
    case "$1" in
        --release) release=${2:?--release requires a value}; shift 2 ;;
        --out) out=${2:?--out requires a value}; shift 2 ;;
        --limit) limit=${2:?--limit requires a value}; shift 2 ;;
        --all) all=true; shift ;;
        --check) check=true; shift ;;
        --repo) repo=${2:?--repo requires a value}; shift 2 ;;
        -h | --help) usage; exit 0 ;;
        *)
            echo "unknown option: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

cd "$repo"
if ! git rev-parse --git-dir >/dev/null 2>&1; then
    echo "not a git repository: $repo" >&2
    exit 1
fi

case "$out" in
    /*) ;;
    *) out="$repo/$out" ;;
esac

repo_url=${CHANGELOG_REPO_URL:-https://github.com/julienfroidefond/stripstream-librarian}
types="feat fix perf refactor docs revert diverge"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

label_for() {
    case "$1" in
        feat) echo "Nouveautés" ;;
        fix) echo "Corrections" ;;
        perf) echo "Performances" ;;
        refactor) echo "Refactoring" ;;
        docs) echo "Documentation" ;;
        revert) echo "Annulations" ;;
        diverge) echo "Divers" ;;
        *) echo "Autres" ;;
    esac
}

# emit_release <title> <date> <revision range>
# Appends the release section to $work/body, or nothing when it has no entries.
emit_release() {
    local title="$1" date="$2" range="$3"
    local buckets="$work/buckets"
    local t sha subject type scope rest body pr prefix line total

    rm -rf "$buckets"
    mkdir -p "$buckets"
    for t in $types chore; do : >"$buckets/$t"; done

    while IFS=$'\t' read -r sha subject; do
        [ -n "$subject" ] || continue
        case "$subject" in
            "chore: bump version to "*) continue ;;
            "chore(release): bump version to "*) continue ;;
        esac
        if [[ $subject =~ ^([a-zA-Z]+)(\(([^\)]+)\))?!?:[[:space:]]+(.*)$ ]]; then
            type=$(printf '%s' "${BASH_REMATCH[1]}" | tr '[:upper:]' '[:lower:]')
            scope=${BASH_REMATCH[3]:-}
            rest=${BASH_REMATCH[4]}
        else
            type=diverge
            scope=""
            rest=$subject
        fi
        case "$type" in
            feat | fix | perf | refactor | docs | revert) ;;
            chore | ci | build | test | style) type=chore ;;
            *) type=diverge ;;
        esac
        if [[ $rest =~ ^(.*)[[:space:]]\(#([0-9]+)\)$ ]]; then
            body=${BASH_REMATCH[1]}
            pr=${BASH_REMATCH[2]}
            rest="$body ([#$pr]($repo_url/pull/$pr))"
        fi
        if [ -n "$scope" ]; then
            prefix="**$scope** — "
        else
            prefix=""
        fi
        line="- $prefix$rest"
        printf '%s\n' "$line" >>"$buckets/$type"
    done < <(git log --no-merges --format='%H%x09%s' "$range")

    total=0
    for t in $types chore; do
        total=$((total + $(wc -l <"$buckets/$t")))
    done
    [ "$total" -gt 0 ] || return 0

    {
        printf '## %s' "$title"
        if [ -n "$date" ]; then printf ' — %s' "$date"; fi
        printf '\n\n'
        for t in $types; do
            if [ -s "$buckets/$t" ]; then
                printf '### %s\n\n' "$(label_for "$t")"
                cat "$buckets/$t"
                printf '\n'
            fi
        done
        if [ -s "$buckets/chore" ]; then
            printf '<details>\n<summary>Maintenance (%s)</summary>\n\n' \
                "$(wc -l <"$buckets/chore" | tr -d ' ')"
            cat "$buckets/chore"
            printf '\n</details>\n\n'
        fi
    } >>"$work/body"
}

: >"$work/body"

latest=$(git tag --list 'v*' --sort=-v:refname | head -1 || true)
if [ -n "$latest" ]; then
    range="$latest..HEAD"
else
    range="HEAD"
fi
if [ -n "$release" ]; then
    emit_release "v$release" "$(date +%Y-%m-%d)" "$range"
else
    emit_release "À venir" "" "$range"
fi

tags=$(git tag --list 'v*' --sort=-v:refname)
count=0
for _ in $tags; do count=$((count + 1)); done

if [ "$all" = true ]; then
    rendered=$count
elif [ "$limit" -lt "$count" ]; then
    rendered=$limit
else
    rendered=$count
fi

i=0
for tag in $tags; do
    [ "$i" -lt "$rendered" ] || break
    if [ "$((i + 1))" -lt "$count" ]; then
        prev=$(printf '%s\n' "$tags" | sed -n "$((i + 2))p")
        range="$prev..$tag"
    else
        range="$tag"
    fi
    date=$(git log -1 --date=short --format='%ad' "$tag")
    emit_release "$tag" "$date" "$range"
    i=$((i + 1))
done

mkdir -p "$(dirname "$out")"
{
    cat <<EOF
---
title: Changelog
description: Historique des versions de Stripstream Librarian
---

<!-- Généré par scripts/generate-changelog.sh — ne pas éditer à la main. -->

Historique des versions publiées, de la plus récente à la plus ancienne.

EOF
    cat "$work/body"
    if [ "$rendered" -lt "$count" ]; then
        printf '*Les versions antérieures sont listées dans les [tags du dépôt](%s/tags).*\n' "$repo_url"
    fi
} >"$work/changelog.md"

if [ "$check" = true ]; then
    if ! diff -u "$out" "$work/changelog.md"; then
        echo "changelog is stale: run scripts/generate-changelog.sh" >&2
        exit 1
    fi
    exit 0
fi

cp "$work/changelog.md" "$out"
echo "wrote $out"
