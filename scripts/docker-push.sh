#!/bin/bash
set -e

REGISTRY="docker.io"
OWNER="julienfroidefond32"
SERVICES=("api" "indexer" "backoffice" "docs")
PLATFORMS="linux/amd64"
BUILDER_NAME="stripstream-multiarch"

# ─── Version bump ───────────────────────────────────────────────────────────
CURRENT_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"

echo "=== Stripstream Librarian Docker Push ==="
echo "Current: $CURRENT_VERSION"
echo "Platforms: $PLATFORMS"
echo ""
echo "  1) patch → $MAJOR.$MINOR.$((PATCH + 1))"
echo "  2) minor → $MAJOR.$((MINOR + 1)).0"
echo "  3) major → $((MAJOR + 1)).0.0"
echo ""
read -rp "Bump [1/2/3]: " BUMP
case "$BUMP" in
    2) NEW_VERSION="$MAJOR.$((MINOR + 1)).0" ;;
    3) NEW_VERSION="$((MAJOR + 1)).0.0" ;;
    *) NEW_VERSION="$MAJOR.$MINOR.$((PATCH + 1))" ;;
esac

echo ""
echo "Version: $CURRENT_VERSION → $NEW_VERSION"
echo ""
echo "Services to build:"
echo "  a) all (${SERVICES[*]})"
for i in "${!SERVICES[@]}"; do
    echo "  $((i + 1))) ${SERVICES[$i]}"
done
echo ""
read -rp "Choice [a/1/2/3/4, comma-separated]: " SVC_CHOICE

if [[ "$SVC_CHOICE" == "a" || -z "$SVC_CHOICE" ]]; then
    SELECTED_SERVICES=("${SERVICES[@]}")
else
    SELECTED_SERVICES=()
    IFS=',' read -ra CHOICES <<< "$SVC_CHOICE"
    for c in "${CHOICES[@]}"; do
        c=$(echo "$c" | tr -d ' ')
        idx=$((c - 1))
        if [[ $idx -ge 0 && $idx -lt ${#SERVICES[@]} ]]; then
            SELECTED_SERVICES+=("${SERVICES[$idx]}")
        fi
    done
fi

echo ""
echo "Building: ${SELECTED_SERVICES[*]}"
echo ""

# Bump version
sed -i.bak "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" Cargo.toml
rm -f Cargo.toml.bak
sed -i.bak "s/\"version\": \"$CURRENT_VERSION\"/\"version\": \"$NEW_VERSION\"/" apps/backoffice/package.json
rm -f apps/backoffice/package.json.bak
cargo update --workspace 2>/dev/null || true

# Commit version bump
git add Cargo.toml Cargo.lock apps/backoffice/package.json
git commit -m "chore: bump version to $NEW_VERSION"
VERSION="$NEW_VERSION"

# ─── Ensure buildx builder exists ──────────────────────────────────────────
if ! docker buildx inspect "$BUILDER_NAME" &>/dev/null; then
    echo "Creating buildx builder: $BUILDER_NAME"
    docker buildx create --name "$BUILDER_NAME" --use --bootstrap
else
    docker buildx use "$BUILDER_NAME"
fi

# ─── Build, tag & push all services (multi-platform) ──────────────────────
# Registry cache speeds up rebuilds: layers shared between builds, persisted across runs.
# api and indexer share apps/api/Dockerfile via different stage targets.
service_dockerfile() {
    case "$1" in
        api|indexer) echo "apps/api/Dockerfile" ;;
        *) echo "apps/$1/Dockerfile" ;;
    esac
}
service_target() {
    case "$1" in
        api) echo "api" ;;
        indexer) echo "indexer" ;;
        *) echo "" ;;
    esac
}
for service in "${SELECTED_SERVICES[@]}"; do
    echo ""
    echo "=== $service (${PLATFORMS}) ==="
    DOCKERFILE=$(service_dockerfile "$service")
    TARGET=$(service_target "$service")
    TARGET_ARG=()
    [ -n "$TARGET" ] && TARGET_ARG=(--target "$TARGET")
    docker buildx build \
        --platform "$PLATFORMS" \
        -f "$DOCKERFILE" \
        "${TARGET_ARG[@]}" \
        -t "$REGISTRY/$OWNER/stripstream-$service:$VERSION" \
        -t "$REGISTRY/$OWNER/stripstream-$service:latest" \
        --cache-from "type=registry,ref=$REGISTRY/$OWNER/stripstream-$service:buildcache" \
        --cache-to "type=registry,ref=$REGISTRY/$OWNER/stripstream-$service:buildcache,mode=max" \
        --push \
        .
    echo "✓ $service pushed ($PLATFORMS)"
done

# ─── Git tag ────────────────────────────────────────────────────────────────
if ! git tag -l "v$VERSION" | grep -q "v$VERSION"; then
    git tag "v$VERSION"
fi

echo ""
echo "=== Done ==="
echo "Version: $VERSION"
echo "Platforms: $PLATFORMS"
echo "Tag: v$VERSION (push with: git push origin v$VERSION)"
