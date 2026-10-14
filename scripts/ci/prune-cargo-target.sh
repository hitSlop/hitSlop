#!/usr/bin/env bash
# Before a Rust cache is saved: keep compiled dependencies, drop what every change rebuilds
# anyway (incremental state, test and binary executables), so caches stay within the
# repository's 10 GB budget.
set -euo pipefail
[ -d target ] || exit 0
find target -type d -name incremental -prune -exec rm -rf {} +
find target -path '*/deps/*' -type f -perm -u+x ! -name '*.so' ! -name '*.dylib' ! -name '*.rlib' ! -name '*.rmeta' -delete
du -sh target
