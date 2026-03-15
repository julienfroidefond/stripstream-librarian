#!/bin/bash
set -e

# Docker Hub
REGISTRY="docker.io"
OWNER="julienfroidefond32"

SERVICES=("api" "indexer" "backoffice")

# ─── Version bump ───────────────────────────────────────────────────────────
CURRENT_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"

echo "=== Stripstream Librarian Docker Push ==="
echo "Current version: $CURRENT_VERSION"
echo ""
echo "Bump version?"
echo "  1) patch  → $MAJOR.$MINOR.$((PATCH + 1))"
echo "  2) minor  → $MAJOR.$((MINOR + 1)).0"
echo "  3) major  → $((MAJOR + 1)).0.0"
echo "  4) skip   → keep $CURRENT_VERSION"
echo ""
read -rp "Choice [1-4]: " BUMP_CHOICE

case "$BUMP_CHOICE" in
    1) NEW_VERSION="$MAJOR.$MINOR.$((PATCH + 1))" ;;
    2) NEW_VERSION="$MAJOR.$((MINOR + 1)).0" ;;
    3) NEW_VERSION="$((MAJOR + 1)).0.0" ;;
    4) NEW_VERSION="$CURRENT_VERSION" ;;
    *) echo "Invalid choice"; exit 1 ;;
esac

if [ "$NEW_VERSION" != "$CURRENT_VERSION" ]; then
    echo "Bumping version: $CURRENT_VERSION → $NEW_VERSION"

    # Update Cargo.toml (workspace version)
    sed -i.bak "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" Cargo.toml
    rm -f Cargo.toml.bak

    # Update backoffice package.json
    sed -i.bak "s/\"version\": \"$CURRENT_VERSION\"/\"version\": \"$NEW_VERSION\"/" apps/backoffice/package.json
    rm -f apps/backoffice/package.json.bak

    # Update Cargo.lock
    cargo update --workspace 2>/dev/null || true

    # Commit version bump
    git add Cargo.toml Cargo.lock apps/backoffice/package.json
    git commit -m "chore: bump version to $NEW_VERSION"
    echo "✓ Version bumped and committed"
else
    echo "Keeping version $CURRENT_VERSION"
fi

VERSION="$NEW_VERSION"

echo ""
echo "Registry: $REGISTRY"
echo "Version: $VERSION"
echo "Services: ${SERVICES[*]}"
echo ""

for service in "${SERVICES[@]}"; do
    echo "Building $service..."
    docker build -f "apps/$service/Dockerfile" -t "$service:latest" .

    echo "Tagging $service..."
    docker tag "$service:latest" "$REGISTRY/$OWNER/stripstream-$service:$VERSION"
    docker tag "$service:latest" "$REGISTRY/$OWNER/stripstream-$service:latest"

    echo "Pushing stripstream-$service:$VERSION..."
    docker push "$REGISTRY/$OWNER/stripstream-$service:$VERSION"

    echo "Pushing stripstream-$service:latest..."
    docker push "$REGISTRY/$OWNER/stripstream-$service:latest"

    echo "✓ $service pushed successfully"
    echo ""
done

echo "=== All images pushed to $REGISTRY ==="
echo "Images:"
for service in "${SERVICES[@]}"; do
    echo "  - $REGISTRY/$OWNER/stripstream-$service:$VERSION"
    echo "  - $REGISTRY/$OWNER/stripstream-$service:latest"
done

# Tag git with version
if ! git tag -l "v$VERSION" | grep -q "v$VERSION"; then
    git tag "v$VERSION"
    echo ""
    echo "Git tag v$VERSION created. Push with: git push origin v$VERSION"
fi
