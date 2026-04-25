#!/bin/bash
set -e

REGISTRY="docker.io"
OWNER="julienfroidefond32"
SERVICES=("api" "indexer" "backoffice" "docs")

# ─── Version bump ───────────────────────────────────────────────────────────
CURRENT_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"

echo "=== Stripstream Librarian Docker Push ==="
echo "Current: $CURRENT_VERSION"
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
read -rp "Choice [a/1/2/3, comma-separated]: " SVC_CHOICE

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

# ─── Build, tag & push all services ─────────────────────────────────────────
for service in "${SELECTED_SERVICES[@]}"; do
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
