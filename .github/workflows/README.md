# CI / CD internals

Reference for the GitHub Actions pipelines. `AGENTS.md` links here instead of inlining
this page: it is not needed to write code, only to understand a pipeline failure.

## Workflows

| Workflow | Trigger | Role |
|----------|---------|------|
| `ci.yml` | pull requests touching `apps/api/**`, `apps/indexer/**`, `crates/**` | `cargo fmt` / `clippy` / `cargo nextest`, PostgreSQL service, `Rust (fmt · clippy · test)` check |
| `ci-web.yml` | pull requests touching the web apps | backoffice and docs validation |
| `deploy.yml` | push to `main` | change detection, image build/push, version bump and deploy |
| `opencode.yml` | `ai` label, `/oc` mentions | OpenCode agent |

`ci.yml` runs on **pull requests only**: the merge to `main` is deliberately not re-validated,
so `Rust (fmt · clippy · test)` MUST stay a required status check on the protected branch.
Direct pushes to `main` bypass the gate — use a PR or `workflow_dispatch` in that case.

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

`Swatinem/rust-cache@v2` needs the `actions: write` permission to **save** its cache; with a
restricted `permissions:` block the cache is restored read-only and every run recompiles the
workspace (~7-8 min of `clippy`, plus the test build). `opencode.yml` therefore declares
`actions: write`; keep it when editing the job. The `issue` and `comment` jobs disable the
job-id key suffix so they share one cache prefix (`add-job-id-key: false`).
