#!/bin/bash
set -e

REGISTRY="docker.io"
OWNER="julienfroidefond32"
SERVICES=("api" "indexer" "backoffice")

# ─── Version bump ───────────────────────────────────────────────────────────
CURRENT_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"
NEW_VERSION="$MAJOR.$MINOR.$((PATCH + 1))"

echo "=== Stripstream Librarian Docker Push ==="
echo "Version: $CURRENT_VERSION → $NEW_VERSION"
echo "Services: ${SERVICES[*]}"
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

# ─── Build, tag & push all services ─────────────────────────────────────────
for service in "${SERVICES[@]}"; do
    echo ""
    echo "=== $service ==="
    docker build -f "apps/$service/Dockerfile" -t "$service:latest" .
    docker tag "$service:latest" "$REGISTRY/$OWNER/stripstream-$service:$VERSION"
    docker tag "$service:latest" "$REGISTRY/$OWNER/stripstream-$service:latest"
    docker push "$REGISTRY/$OWNER/stripstream-$service:$VERSION"
    docker push "$REGISTRY/$OWNER/stripstream-$service:latest"
    echo "✓ $service pushed"
done

# ─── Git tag ────────────────────────────────────────────────────────────────
if ! git tag -l "v$VERSION" | grep -q "v$VERSION"; then
    git tag "v$VERSION"
fi

echo ""
echo "=== Done ==="
echo "Version: $VERSION"
echo "Tag: v$VERSION (push with: git push origin v$VERSION)"
