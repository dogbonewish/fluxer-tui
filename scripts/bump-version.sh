#!/bin/bash
#[major|minor|patch|VERSION]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
CARGO_TOML="$ROOT_DIR/Cargo.toml"

if [ ! -f "$CARGO_TOML" ]; then
    echo "toml not found at $CARGO_TOML"
    exit 1
fi

# get latest version
CURRENT_VERSION=$(grep '^version = ' "$CARGO_TOML" | head -1 | sed 's/version = "\(.*\)"/\1/')

if [ -z "$CURRENT_VERSION" ]; then
    echo "idk the current version"
    exit 1
fi

# Determine new version
if [ $# -eq 0 ]; then
    echo "current version: $CURRENT_VERSION"
    echo "usage: $0 [major|minor|patch|VERSION]"
    exit 1
fi

case "$1" in
    major)
        NEW_VERSION=$(echo "$CURRENT_VERSION" | awk -F. '{$1=$1+1; $2=0; $3=0; print $1"."$2"."$3}')
        ;;
    minor)
        NEW_VERSION=$(echo "$CURRENT_VERSION" | awk -F. '{$2=$2+1; $3=0; print $1"."$2"."$3}')
        ;;
    patch)
        NEW_VERSION=$(echo "$CURRENT_VERSION" | awk -F. '{$3=$3+1; print $1"."$2"."$3}')
        ;;
    *)
        NEW_VERSION="$1"
        ;;
esac

# validation
if ! echo "$NEW_VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
    echo "wrong version format: $NEW_VERSION (i was expecting X.Y.Z)"
    exit 1
fi

# update cargo.toml
sed -i "s/^version = \".*\"/version = \"$NEW_VERSION\"/" "$CARGO_TOML"

echo "bumped v: $CURRENT_VERSION -> $NEW_VERSION"
echo ""
echo "next:"
echo "  git add Cargo.toml Cargo.lock"
echo "  git commit -m \"version bubmped to v$NEW_VERSION\""
echo "  git tag v$NEW_VERSION"
echo "  git push origin main v$NEW_VERSION"
