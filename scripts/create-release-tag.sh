#!/usr/bin/env bash
set -euo pipefail

# Create and push a release tag based on Cargo.toml version
# Usage: scripts/create-release-tag.sh [--no-push]

NO_PUSH=false
if [ "${1:-}" = "--no-push" ]; then
  NO_PUSH=true
fi

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

if [ ! -f Cargo.toml ]; then
  echo "Cargo.toml not found. Run from repo root." >&2
  exit 1
fi

VERSION=$(grep -E '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
if [ -z "$VERSION" ]; then
  echo "Failed to parse version from Cargo.toml" >&2
  exit 1
fi

TAG="v$VERSION"
echo -e "${BLUE}Preparing release tag for cigen ${VERSION}${NC}"

if git tag --list | grep -q "^${TAG}$"; then
  echo -e "${YELLOW}✓ Tag ${TAG} already exists${NC}"
  commit=$(git rev-list -n 1 "${TAG}")
  echo "  Points to: $(git rev-parse --short "$commit") - $(git log --format=%s -n 1 "$commit")"
else
  echo -e "${GREEN}Creating tag ${TAG}${NC}"
  git tag -a "$TAG" -m "Release cigen ${VERSION}"
  echo "  Created at current commit: $(git rev-parse --short HEAD)"
fi

if [ "$NO_PUSH" = false ]; then
  echo -e "${BLUE}Pushing tag ${TAG} to origin...${NC}"
  git push origin "$TAG"
  echo -e "${GREEN}✓ Tag pushed. GitHub release will be created by workflow.${NC}"
else
  echo -e "${YELLOW}Skipping push (--no-push). Push manually with:${NC}"
  echo "  git push origin ${TAG}"
fi

