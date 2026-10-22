# CI / CD internals

Reference for the GitHub Actions pipelines. `AGENTS.md` links here instead of inlining
this page: it is not needed to write code, only to understand a pipeline failure.

## Workflows

| Workflow | Trigger | Role |
|----------|---------|------|
| `ci.yml` | every pull request, push to `main` (cache warm-up only) | `cargo fmt` / `clippy` / `cargo nextest`, PostgreSQL service, `Rust (fmt · clippy · test)` check |
| `ci-web.yml` | every pull request | backoffice and docs validation |
| `dependabot-automerge.yml` | Dependabot pull requests | enables native auto-merge for patch/minor bumps |
| `deploy.yml` | push to `main` | change detection, image build/push, version bump and deploy |
| `opencode.yml` | `ai` label, `/oc` mentions | OpenCode agent |

The merge to `main` is deliberately **not re-validated**, so the PR checks must stay
**required** on the protected branch: `Rust (fmt · clippy · test)`, `Backoffice (tsc · lint ·
tests)` and `Docs (build)`. Direct pushes to `main` bypass the gate — use a PR or
`workflow_dispatch` in that case.

Required checks are the reason `ci.yml` and `ci-web.yml` **trigger on every PR** instead of
using `on.pull_request.paths`: a workflow filtered out at the trigger level never reports, so
its required check stays "Expected" and the PR can never merge (a docs-only PR would hang
forever on the Rust check). The path filter therefore lives inside each workflow
(`dorny/paths-filter` in a `changes` job); GitHub counts a job skipped by `if:` as a
successful required check. The `gate` job in `ci.yml` mirrors that rule: it fails on any real
failure and accepts `skipped` when no Rust file changed.

## Rust gate (`ci.yml`)

Three jobs, all racing each other:

| Job | What it does |
|-----|--------------|
| `lint` | `cargo fmt --check` + `clippy --all-targets -D warnings` |
| `test` | `cargo nextest`, sharded over a 2-entry matrix (`--partition count:<shard>/<job-total>`) |
| `gate` | nothing (~2 s), only aggregates the two above |

Only `gate` matters to branch protection: its name is the check name
(`Rust (fmt · clippy · test)`), so the real work can be split into as many jobs as we want
without ever touching the ruleset. Keep that name stable.

Times measured on this repo (1371 tests, 585 of them `#[sqlx::test]`): 11m52 for the old
single-job setup, 6m32 cold (no cache) and **4m31 warm** with this one.

Two non-obvious settings, both worth ~6 min:

- **PostgreSQL tuning.** `#[sqlx::test]` creates a throwaway database per test and replays all
  ~130 migrations on it, one transaction each: 585 tests × ~150 fsyncs is ~88 % of the suite.
  The job sets `synchronous_commit=off` and `full_page_writes=off` (migration replay
  0.55 s → 0.21 s per database). This **cannot** be done through the service container's
  `options:` field: it is spliced into `docker create` *before* the image name, so `-c fsync=off`
  is parsed as a docker flag and the container never starts. Hence the
  `ALTER SYSTEM ... / pg_reload_conf()` step going through
  `docker exec "${{ job.services.postgres.id }}"`. Both settings are reload-safe, so no restart.
- **Parallelism.** `lint` and `test` are separate jobs, and `nextest` is sharded. Partitioning is
  by *test count*, not duration, so the busiest shard varies between ~135 s and ~210 s.

The `rust` filter of the `changes` job also lists `.github/workflows/ci.yml`, so a workflow
edit is itself validated. (This used to be an `on.pull_request.paths` entry; note that GitHub
evaluates that filter against the **default branch** version of the file, so a PR touching
only `ci.yml` ran nothing until the filter existed on `main` — another reason it now lives in
the job.)

## Dependabot auto-merge

`dependabot-automerge.yml` runs on `pull_request_target` (the workflow comes from `main`, so
it also applies to branches created before this file existed), restricted to
`dependabot[bot]`. It reads `dependabot/fetch-metadata`'s `updated-dependencies-json` and only
merges when **every** bumped dependency is a patch, or a minor on a post-1.0 package.
`0.x → 0.y` counts as major (breaking per Cargo/npm convention), so it stays manual — same for
real majors. No PR code is checked out, so `pull_request_target` carries no injection surface
here.

It does **not** use GitHub's native auto-merge: the `enablePullRequestAutoMerge` mutation
answers `Resource not accessible by personal access token` for fine-grained PATs, whatever the
repository permissions (verified with `admin: true` on this repo). The workflow therefore polls
the three required checks itself, approves the PR, and runs `gh pr merge --squash
--delete-branch` once they are all `SUCCESS`/`SKIPPED`, up to 60 minutes. A new push on the PR
cancels the pending run and starts over.

The approval is required by the ruleset's `require_extra_approval_for_unattributed_changes`:
a Dependabot push is unattributed, so the PR stays `BLOCKED` with zero required reviews until
someone approves. It is only submitted after the three checks are green, and it is the same
approval a human would give. GitHub then recomputes mergeability asynchronously (the PR keeps
reporting `BLOCKED` for ~20 s), so the workflow waits for that state to clear before merging,
and retries the merge a few times.

`AUTOMERGE_TOKEN` (fine-grained PAT, `Contents: write`, `Pull requests: write`) is required:
a merge with the repository `GITHUB_TOKEN` does not trigger `push` workflows, so `deploy.yml`
— version bump, image build, deployment — would silently not run. The PAT attributes the
merge to its owner, like a human merge.

## Images

The API and indexer share a single Dockerfile (`apps/api/Dockerfile`) with two targets
(`--target api` / `--target indexer`). The build uses **cargo-chef** (planner →
`recipe.json` → cooked deps layer), the **mold** linker, and a separate `sqlx-installer`
stage so app rebuilds never recompile `sqlx-cli`.

`deploy.yml` does per-service change detection (`git diff HEAD~1`), conditional build with
registry cache, and pushes the images from GitHub-hosted runners. A `deploy` job then runs on
the self-hosted `mac-mini` runner to pull the images and restart the stack from the local
`docker-stack` checkout.

`deploy.yml` also owns versioning: when a push to `main` produces at least one image, it bumps
the workspace version (patch by default; `minor` / `major` / `none` via the `workflow_dispatch`
input), commits `chore: bump version to X`, tags `vX`, and pushes both to `main` before
building the images under the new version. The push uses a write **deploy key** that is a
bypass actor on the "Protect main" ruleset (which otherwise requires a pull request); the job
skips commits whose message starts with `chore: bump version to`, which prevents the push from
retriggering itself forever.

Two guards keep the counter honest: the bump is keyed on the version `main` actually carries,
so a re-run reuses it instead of bumping twice; and a tag that already exists without its
commit on `main` is skipped (the version moves to the next patch), because tags are immutable.
The commit is pushed **before** the tag and a failed push now fails the job — a warning there
is how an orphan `v3.32.52` tag froze the counter for two days (2026-10-20). `scripts/docker-push.sh` still exists for local builds, but its
interactive version bump is redundant with the workflow.

## Deploy runner: keychain and compose (gotcha)

On the self-hosted `mac-mini` runner, OrbStack's `docker` CLI ignores `DOCKER_CONFIG` /
`--config` and always reads `credsStore: osxkeychain` from `~/.docker/config.json`, which is
unreachable from the runner's non-interactive launchd session. The `deploy` jobs prepend a stub
`docker-credential-osxkeychain` (reports "no credentials") to `PATH` so public image pulls run
anonymously.

Do **not** set `DOCKER_CONFIG` in the runner env: it hides the `compose` CLI plugin
(`$DOCKER_CONFIG/cli-plugins`), breaking `./scripts/stack.sh up`.

## Cargo cache

`Swatinem/rust-cache@v2` saves by default (`save-if: true`), but cache entries are
branch-scoped: what a **pull request** saves lands under `refs/pull/<n>/merge` and is only
restorable by that same PR. A PR restores from its own ref *and from its base branch*, so
without a cache on `main` every PR recompiles ~450 crates twice (clippy, then the test profile).

`ci.yml` therefore also runs on `push: main`, in *warm-up* mode: `fmt`, `clippy` and
`nextest --no-run` execute (so the cache is populated), but no test is run. That run is what
every subsequent PR restores. Both jobs restrict saving with
`save-if: ${{ github.ref == 'refs/heads/main' }}` — note the `${{ }}`: the action compares the
raw input string to `"true"`, so an unwrapped expression silently disables saving. The two jobs
use distinct `shared-key:` values (`lint`, `test`) because they produce different artifacts.

`Swatinem/rust-cache@v2` also needs the `actions: write` permission to **save** its cache; with a
restricted `permissions:` block the cache is restored read-only and every run recompiles the
workspace (~7-8 min of `clippy`, plus the test build). `opencode.yml` therefore declares
`actions: write`; keep it when editing the job. The `issue` and `comment` jobs disable the
job-id key suffix so they share one cache prefix (`add-job-id-key: false`).
