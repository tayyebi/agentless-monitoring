#!/bin/bash

# Release script for Agentless Monitor
# Usage: ./scripts/release.sh <version>
# Example: ./scripts/release.sh v1.0.0

set -e

if [ $# -eq 0 ]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 v1.0.0"
    exit 1
fi

VERSION=$1
CURRENT_VERSION=$(grep '^version = ' Cargo.toml | cut -d'"' -f2)

echo "🚀 Creating release $VERSION"
echo "Current version: $CURRENT_VERSION"

# Validate version format
if [[ ! $VERSION =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "❌ Invalid version format. Use format: v1.0.0"
    exit 1
fi

# Extract version number without 'v' prefix
VERSION_NUMBER=${VERSION#v}

# Update Cargo.toml version
echo "📝 Updating Cargo.toml version to $VERSION_NUMBER"
sed -i "s/^version = \".*\"/version = \"$VERSION_NUMBER\"/" Cargo.toml

# Update any version references in README or other files
echo "📝 Updating version references in documentation"
# Add any other files that reference the version here

# Commit changes
echo "💾 Committing version update"
git add Cargo.toml
git commit -m "chore: bump version to $VERSION"

# Create and push tag
echo "🏷️ Creating and pushing tag $VERSION"
git tag -a "$VERSION" -m "Release $VERSION"
git push origin main
git push origin "$VERSION"

echo "✅ Release $VERSION created successfully!"
echo "🔗 GitHub Actions will now build and publish the release"
echo "📦 Check the Actions tab for build progress: https://github.com/tayyebi/agentless-monitoring/actions"

# Optional: Open the releases page
if command -v xdg-open > /dev/null; then
    xdg-open "https://github.com/tayyebi/agentless-monitoring/releases"
elif command -v open > /dev/null; then
    open "https://github.com/tayyebi/agentless-monitoring/releases"
fi
