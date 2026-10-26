# Auto-merge eligibility of a Dependabot pull request.
#
# Used by `.github/workflows/dependabot-automerge.yml`; policy in
# `.github/dependabot-policy.md`.
#
# Inputs (--argjson):
#   $deps   `updated-dependencies-json` from dependabot/fetch-metadata
#   $files  the PR's changed files (`GET /repos/{owner}/{repo}/pulls/{n}/files`),
#           `[]` when the caller did not fetch them
#
# Output: {"eligible": bool, "reason": string}
#
# Rules, in order of application (one decision is taken for the whole PR, so
# every bumped dependency must qualify — Dependabot groups several bumps into a
# single PR):
#   - patch, or minor of a post-1.0 package: eligible;
#   - `0.x -> 0.y`: never (breaking by Cargo/npm convention, see the policy) —
#     checked before the minor rule because Cargo treats a pre-1.0 minor as a
#     major;
#   - major: never;
#   - image digest bump: Dependabot reports no `updateType` at all when the tag
#     is not semver (`lukemathwalker/cargo-chef:latest-rust-1-bookworm`), so the
#     `93aa3e1 -> b75e76d` digest bumps landed in the "unknown" bucket and always
#     stayed manual. They are eligible now, but only on proof from the diff:
#     every changed line is an `image@sha256:` reference and the same `repo:tag`
#     sits on both sides — i.e. only the digest moved. A tag change under a
#     non-semver tag (`latest-rust-1-bookworm` -> `latest-rust-1-trixie`) moves
#     the ref and is therefore rejected;
#   - anything else: manual.

# One dependency of the PR.
def decision:
  if .updateType == "version-update:semver-major" then "major"
  elif .updateType == "version-update:semver-patch" then "auto"
  elif ((.prevVersion // "") | test("^0\\.")) then "0.x"
  elif .updateType == "version-update:semver-minor" then
    # Grouped PRs carry no `prevVersion`; groups are configured as minor/patch
    # only, with pre-1.0 direct dependencies excluded (`.github/dependabot.yml`).
    (if ((.prevVersion // "") != "") or ((.dependencyGroup // "") != "")
     then "auto" else "unknown" end)
  elif ((.updateType // "") == "")
       and ((.packageEcosystem // "") | startswith("docker")) then "digest"
  else "unknown" end;

# `repo:tag@sha256:<64 hex>` — the only line shape a digest bump may touch.
def image_ref:
  capture("(?<image>[A-Za-z0-9._/-]+:[A-Za-z0-9._-]+)@sha256:[0-9a-f]{64}")
  | .image;

# Every added/removed line of one file's patch, tagged with its `ref` (null when
# the line is not an image digest reference).
def digest_changes:
  [ .patch // ""
    | split("\n")[]
    | select(test("^[+-]"))
    | .[0:1] as $sign
    | .[1:] as $body
    | { sign: $sign,
        ref: (if ($body | test("@sha256:[0-9a-f]{64}"))
              then ($body | image_ref) else null end) }
  ];

# True when the whole diff is a digest-only move: no line without a digest
# reference, at least one removal and one addition, and the same set of
# `repo:tag` on both sides.
def digest_only:
  [ .[] | digest_changes[] ] as $lines
  | ($lines | length) > 0
    and ($lines | all(.ref != null))
    and (([$lines[] | select(.sign == "-")] | length) > 0)
    and (([$lines[] | select(.sign == "+")] | length) > 0)
    and (([$lines[] | select(.sign == "-") | .ref] | sort)
         == ([$lines[] | select(.sign == "+") | .ref] | sort));

($deps | map(decision)) as $decisions
| (($decisions | all(. == "auto" or . == "digest"))
   and ($files | length) > 0
   and ($files | digest_only)) as $digest
| if ($decisions | all(. == "auto")) then
    { eligible: true, reason: "patch/minor bump" }
  elif $digest then
    { eligible: true, reason: "digest-only image bump" }
  elif ($decisions | any(. == "major")) then
    { eligible: false, reason: "major bump, manual review required" }
  elif ($decisions | any(. == "0.x")) then
    { eligible: false, reason: "0.x -> 0.y bump, manual review required" }
  elif ($decisions | any(. == "digest")) then
    { eligible: false,
      reason: "image digest bump that changes more than a digest, manual review required" }
  else
    { eligible: false, reason: "unrecognised update type, manual review required" }
  end
