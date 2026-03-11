#!/bin/bash
set -e

# Docker Hub
REGISTRY="docker.io"
OWNER="julienfroidefond32"
VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')

SERVICES=("api" "indexer" "backoffice")

echo "=== Stripstream Librarian Docker Push ==="
echo "Registry: $REGISTRY"
echo "Version: $VERSION"
echo "Services: ${SERVICES[@]}"
echo ""

for service in "${SERVICES[@]}"; do
    echo "Building $service..."
    docker build -f apps/$service/Dockerfile -t $service:latest .
    
    echo "Tagging $service..."
    docker tag $service:latest $REGISTRY/$OWNER/stripstream-$service:$VERSION
    docker tag $service:latest $REGISTRY/$OWNER/stripstream-$service:latest
    
    echo "Pushing stripstream-$service:$VERSION..."
    docker push $REGISTRY/$OWNER/stripstream-$service:$VERSION
    
    echo "Pushing stripstream-$service:latest..."
    docker push $REGISTRY/$OWNER/stripstream-$service:latest
    
    echo "✓ $service pushed successfully"
    echo ""
done

echo "=== All images pushed to $REGISTRY ==="
echo "Images:"
for service in "${SERVICES[@]}"; do
    echo "  - $REGISTRY/$OWNER/stripstream-$service:$VERSION"
    echo "  - $REGISTRY/$OWNER/stripstream-$service:latest"
done
