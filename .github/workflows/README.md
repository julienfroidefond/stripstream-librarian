# CI / CD internals

Reference for the GitHub Actions pipelines. `AGENTS.md` links here instead of inlining
this page: it is not needed to write code, only to understand a pipeline failure.

## Workflows

| Workflow | Trigger | Role |
|----------|---------|------|
| `ci.yml` | pull requests touching Rust paths, push to `main` (cache warm-up only) | `cargo fmt` / `clippy` / `cargo nextest`, PostgreSQL service, `Rust (fmt · clippy · test)` check |
| `ci-web.yml` | pull requests touching the web apps | backoffice and docs validation |
| `deploy.yml` | push to `main` | change detection, image build/push, version bump and deploy |
| `opencode.yml` | `ai` label, `/oc` mentions | OpenCode agent |

The merge to `main` is deliberately **not re-validated**, so `Rust (fmt · clippy · test)` MUST
stay a required status check on the protected branch. Direct pushes to `main` bypass the gate —
use a PR or `workflow_dispatch` in that case.

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

`paths:` also lists `.github/workflows/ci.yml`, so a workflow edit is itself validated. Note that
GitHub evaluates `paths` against the **default branch** version of the file: a PR that only
touches `ci.yml` will run nothing until this filter exists on `main`.

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
retriggering itself forever. `scripts/docker-push.sh` still exists for local builds, but its
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

`Swatinem/rust-cache@v2` only **saves** on the default branch, and cache entries are
branch-scoped: with a PR-only trigger the repo had no cache at all, and every PR recompiled
~450 crates twice (clippy, then the test profile).

`ci.yml` therefore also runs on `push: main`, in *warm-up* mode: `fmt`, `clippy` and
`nextest --no-run` execute (so the cache is populated), but no test is run. That run is what
every subsequent PR restores. `save-if: github.ref == 'refs/heads/main'` is spelled out
explicitly in both jobs so it does not look like a missing `save-if: true`. The two jobs use
distinct `shared-key:` values (`lint`, `test`) because they produce different artifacts.

`Swatinem/rust-cache@v2` also needs the `actions: write` permission to **save** its cache; with a
restricted `permissions:` block the cache is restored read-only and every run recompiles the
workspace (~7-8 min of `clippy`, plus the test build). `opencode.yml` therefore declares
`actions: write`; keep it when editing the job. The `issue` and `comment` jobs disable the
job-id key suffix so they share one cache prefix (`add-job-id-key: false`).
