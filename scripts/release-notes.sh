#!/usr/bin/env bash
# Print one version's section of CHANGELOG.md (without its heading). Usage: release-notes.sh 0.6.0
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
notes=$(awk -v v="$1" '
  /^## \[/ { if (on) exit; on = index($0, "## [" v "]") == 1; next }
  /^\[.*\]: / { exit }
  on
' CHANGELOG.md)
if [ -z "${notes//[[:space:]]/}" ]; then
  echo "error: no CHANGELOG.md section for $1" >&2
  exit 1
fi
printf '%s\n' "$notes"
