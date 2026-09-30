#!/bin/sh
set -eu
repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
export PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"
exec bun "$repo_root/scripts/embed-templates.ts" "${1:?app bundle path required}"
