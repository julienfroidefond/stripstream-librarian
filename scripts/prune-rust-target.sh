#!/usr/bin/env bash
# Prune Rust build caches for this repository and its git worktrees.
#
# `target/*/incremental` is the largest and fastest-regrowing part of a Cargo
# target directory (several GB on this workspace). It is safe to delete: Cargo
# recompiles the workspace crates on the next build, while the dependency
# artifacts in `target/*/deps` stay valid.
#
# Dry run by default. `--stale-days N` also delegates older-artifact pruning to
# cargo-sweep (install with `cargo install cargo-sweep`), which understands
# Cargo's fingerprints and is safer than deleting files from deps/ by hand.

set -euo pipefail

APPLY=0
STALE_DAYS=""
ROOTS=()

usage() {
  cat <<'EOF'
Usage: scripts/prune-rust-target.sh [OPTIONS] [PATH...]

Report (and optionally prune) the target/ directories of this repository and
its git worktrees. Without PATH arguments, worktrees are discovered with
`git worktree list`.

Options:
  -a, --apply          delete files (default: dry run)
  -t, --stale-days N   also prune artifacts unused for N days (cargo-sweep)
  -h, --help           show this help

Examples:
  scripts/prune-rust-target.sh                  # report only
  scripts/prune-rust-target.sh --apply          # drop incremental caches
  scripts/prune-rust-target.sh --apply -t 30    # + artifacts older than 30 days
EOF
}

while (($# > 0)); do
  case "$1" in
    -a | --apply)
      APPLY=1
      ;;
    -t | --stale-days)
      (($# >= 2)) || {
        echo "error: --stale-days requires a value" >&2
        exit 2
      }
      STALE_DAYS=$2
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    --)
      shift
      ROOTS+=("$@")
      break
      ;;
    -*)
      echo "error: unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
    *)
      ROOTS+=("$1")
      ;;
  esac
  shift
done

if [[ -n "$STALE_DAYS" ]] && ! [[ "$STALE_DAYS" =~ ^[0-9]+$ ]]; then
  echo "error: --stale-days must be a non-negative integer" >&2
  exit 2
fi

if [[ -n "$STALE_DAYS" ]] && ! command -v cargo-sweep >/dev/null 2>&1; then
  echo "error: cargo-sweep is required for --stale-days; install it with:" >&2
  echo "  cargo install cargo-sweep" >&2
  exit 1
fi

if ((${#ROOTS[@]} == 0)); then
  script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
  repo_root=$(git -C "$script_dir" rev-parse --show-toplevel 2>/dev/null) || {
    echo "error: not inside a git repository; pass explicit PATH arguments" >&2
    exit 1
  }
  while IFS= read -r line; do
    case "$line" in
      "worktree "*) ROOTS+=("${line#worktree }") ;;
    esac
  done < <(git -C "$repo_root" worktree list --porcelain)
fi

found=0
for root in "${ROOTS[@]}"; do
  target="$root/target"
  [[ -d "$target" ]] || continue
  found=1

  echo "==> $target"
  du -sh "$target" | awk '{printf "    %-22s %s\n", "total", $1}'

  for incremental in "$target"/*/incremental; do
    [[ -d "$incremental" ]] || continue
    profile=$(basename "$(dirname "$incremental")")
    size=$(du -sh "$incremental" | awk '{print $1}')
    if ((APPLY)); then
      rm -rf "$incremental"
      printf '    %-22s %s (removed)\n' "$profile/incremental" "$size"
    else
      printf '    %-22s %s (would remove)\n' "$profile/incremental" "$size"
    fi
  done

  if [[ -n "$STALE_DAYS" ]]; then
    echo "    sweeping artifacts older than ${STALE_DAYS}d with cargo-sweep"
    if ((APPLY)); then
      (cd "$root" && cargo sweep --time "$STALE_DAYS")
    else
      (cd "$root" && cargo sweep --dry-run --time "$STALE_DAYS")
    fi
  fi
done

if ((found == 0)); then
  echo "No target/ directory found."
fi
