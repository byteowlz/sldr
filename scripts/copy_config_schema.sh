#!/usr/bin/env bash
set -euo pipefail

SRC_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC_SCHEMAS="$SRC_DIR/examples/schemas"
DEST_DIR="$SRC_DIR/../schemas/sldr"

if [ ! -d "$SRC_SCHEMAS" ]; then
  echo "Source schemas not found: $SRC_SCHEMAS" >&2
  exit 1
fi

mkdir -p "$DEST_DIR"

# Copy all schema files
for schema in "$SRC_SCHEMAS"/*.json; do
  dest_name=$(basename "$schema")
  cp "$schema" "$DEST_DIR/$dest_name"
  echo "Copied $dest_name"
done

echo "All schemas copied to $DEST_DIR"

# Commit and push if in a git repo
if [ -d "$DEST_DIR/../.git" ]; then
  cd "$DEST_DIR/.."
  git pull --rebase 2>/dev/null || true
  git add sldr/
  git commit -m "feat: updated sldr schemas" || echo "No changes to commit"
  git push || echo "Push failed (check remote access)"
  echo "Committed and pushed schema changes"
fi
