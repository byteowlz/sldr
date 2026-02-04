#!/usr/bin/env bash
set -euo pipefail

SRC_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC_SCHEMA_DIR="$SRC_DIR/examples/schemas"
DEST_DIR="$SRC_DIR/../schemas/sldr"

if [ ! -d "$SRC_SCHEMA_DIR" ]; then
  echo "Source schema directory not found: $SRC_SCHEMA_DIR" >&2
  exit 1
fi

# Generate schemas first if they don't exist
if [ ! -f "$SRC_SCHEMA_DIR/sldr.config.schema.json" ]; then
  echo "Generating schemas..."
  cd "$SRC_DIR"
  just schemas
fi

mkdir -p "$DEST_DIR"

# Copy all schemas
echo "Copying schemas to $DEST_DIR"
cp "$SRC_SCHEMA_DIR"/*.json "$DEST_DIR/"

cd "$DEST_DIR"

# Check if we're in a git repository
if git rev-parse --git-dir > /dev/null 2>&1; then
  git add .
  echo "Staged schema changes in $DEST_DIR"
  echo "Commit and push with:"
  echo "  git commit -m 'feat: update sldr schemas'"
  echo "  git push"
else
  echo "Not in a git repository, schemas copied but not committed"
fi

echo "Done!"
