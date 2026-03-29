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

# ─── Detect changed services ─────────────────────────────────────────────────
LAST_TAG=$(git tag -l "v*" --sort=-v:refname | head -1)
CHANGED_SERVICES=()

if [ -n "$LAST_TAG" ]; then
    CHANGED_FILES=$(git diff --name-only "$LAST_TAG"..HEAD 2>/dev/null || echo "")
    CRATES_CHANGED=false

    if echo "$CHANGED_FILES" | grep -q "^crates/"; then
        CRATES_CHANGED=true
    fi

    for service in "${SERVICES[@]}"; do
        if echo "$CHANGED_FILES" | grep -q "^apps/$service/"; then
            CHANGED_SERVICES+=("$service")
        elif [ "$CRATES_CHANGED" = true ] && [ "$service" != "backoffice" ]; then
            # Shared crates affect api and indexer, not backoffice
            CHANGED_SERVICES+=("$service")
        fi
    done

    # Also rebuild if root Cargo.toml/Cargo.lock/Dockerfile changed
    if echo "$CHANGED_FILES" | grep -qE "^(Cargo\.(toml|lock)|docker-compose)"; then
        for service in "${SERVICES[@]}"; do
            if [[ ! " ${CHANGED_SERVICES[*]} " =~ " $service " ]]; then
                CHANGED_SERVICES+=("$service")
            fi
        done
    fi
else
    # No previous tag → build everything
    CHANGED_SERVICES=("${SERVICES[@]}")
fi

# If nothing changed, still allow force-building all
if [ ${#CHANGED_SERVICES[@]} -eq 0 ]; then
    echo "No services changed since $LAST_TAG."
    read -rp "Build all anyway? [y/N]: " FORCE_ALL
    if [[ "$FORCE_ALL" =~ ^[Yy]$ ]]; then
        CHANGED_SERVICES=("${SERVICES[@]}")
    else
        echo "Nothing to push."
        exit 0
    fi
fi

echo ""
echo "Registry: $REGISTRY"
echo "Version: $VERSION"
echo "Last tag: ${LAST_TAG:-none}"
echo "Services to build: ${CHANGED_SERVICES[*]}"
echo "Skipped: $(for s in "${SERVICES[@]}"; do [[ ! " ${CHANGED_SERVICES[*]} " =~ " $s " ]] && echo -n "$s "; done)"
echo ""
read -rp "Continue? [Y/n]: " CONFIRM
if [[ "$CONFIRM" =~ ^[Nn]$ ]]; then
    echo "Aborted."
    exit 0
fi

for service in "${CHANGED_SERVICES[@]}"; do
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

echo "=== Push complete ==="
echo "Built & pushed:"
for service in "${CHANGED_SERVICES[@]}"; do
    echo "  ✓ $REGISTRY/$OWNER/stripstream-$service:$VERSION"
done
SKIPPED=""
for s in "${SERVICES[@]}"; do
    [[ ! " ${CHANGED_SERVICES[*]} " =~ " $s " ]] && SKIPPED="$SKIPPED $s"
done
if [ -n "$SKIPPED" ]; then
    echo "Skipped (unchanged):$SKIPPED"
fi

# Tag git with version
if ! git tag -l "v$VERSION" | grep -q "v$VERSION"; then
    git tag "v$VERSION"
    echo ""
    echo "Git tag v$VERSION created. Push with: git push origin v$VERSION"
fi
