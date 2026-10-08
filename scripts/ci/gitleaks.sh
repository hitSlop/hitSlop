#!/usr/bin/env bash
set -euo pipefail

# Scan commits, not the final diff: an introduced secret must still fail after removal.
base="${GITLEAKS_BASE:-}"
range=""
if [[ -n "$base" && ! "$base" =~ ^0+$ ]]; then
  if [[ "${GITLEAKS_EVENT:-}" == pull_request ]]; then
    base=$(git merge-base HEAD "$base")
  else
    git cat-file -e "$base^{commit}"
  fi
  range="$base..HEAD"
  # A scanner/rules change can expose secrets in older commits. Check even reverted edits.
  config_changes=$(git log --diff-merges=first-parent --format= --name-only "$range" -- .gitleaks.toml .gitleaksignore .github/workflows/secret-scan.yml scripts/ci/gitleaks.sh)
  if [[ -n "$config_changes" ]]; then
    range=""
  fi
fi

args=(git --redact --no-banner)
if [[ -n "$range" ]]; then
  echo "Scanning introduced commits: $range"
  args+=(--log-opts="--full-history --diff-merges=first-parent $range")
else
  echo "Scanning full Git history"
  args+=(--log-opts="--full-history --all --diff-merges=first-parent")
fi
"${GITLEAKS_BIN:?Set GITLEAKS_BIN to the pinned Gitleaks executable}" "${args[@]}"
