<!-- rtk-instructions v2 -->
# RTK (Rust Token Killer) — token-optimized commands

`rtk` is a generic CLI wrapper that compresses command output (60-90% fewer tokens). It is
installed on the developer machine (`/opt/homebrew/bin/rtk`), **not** on the CI runners.

**Golden rule**: when `rtk` is available (`command -v rtk`), prefix every command with it, even
inside `&&` chains: `rtk cargo clippy && rtk git add .`. If a filter does not exist, `rtk` passes
the command through unchanged, so it is always safe. In CI, run the bare command instead.

```bash
# Build · Test · Git · GitHub
rtk cargo build | rtk cargo check | rtk cargo clippy | rtk cargo test
rtk tsc | rtk lint | rtk prettier --check | rtk next build
rtk vitest | rtk playwright test | rtk test <cmd>
rtk git status|log|diff|show|add|commit|push|pull|branch|fetch|stash|worktree
rtk gh pr view|checks | rtk gh run list | rtk gh issue list | rtk gh api

# JS/TS · Files · Analysis · Infra · Network
rtk npm run <script> | rtk npx <cmd> | rtk pnpm install|list|outdated | rtk prisma
rtk ls <path> | rtk read <file> | rtk grep <pattern> | rtk find <pattern>
rtk err <cmd> | rtk log <file> | rtk json <file> | rtk deps | rtk env | rtk summary <cmd> | rtk diff
rtk docker ps|images|logs | rtk kubectl get|logs | rtk curl <url> | rtk wget <url>

# Meta
rtk gain [--history] | rtk discover | rtk proxy <cmd> | rtk init [--global]
```

Project rules live in `AGENTS.md`; this file only documents the wrapper.

<!-- /rtk-instructions -->
