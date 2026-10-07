#!/bin/sh
set -eu

# Named commands need this helper in every app configuration, including Xcode Run.
repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
export PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"
app=${1:?app bundle path required}
profile=${HITSLOP_CARGO_PROFILE:-release}
(cd "$repo_root" && bun scripts/build/core.ts --engine)
evaluator="$repo_root/target/$profile/hitslop-evaluator"
if [ ! -x "$evaluator" ]; then
  echo "hitslop-evaluator is missing at $evaluator" >&2
  exit 70
fi
/bin/mkdir -p "$app/Contents/Helpers"
/bin/cp "$evaluator" "$app/Contents/Helpers/hitslop-evaluator"
/bin/chmod 755 "$app/Contents/Helpers/hitslop-evaluator"
/usr/bin/codesign --force --options runtime --sign "${EXPANDED_CODE_SIGN_IDENTITY:--}" "$app/Contents/Helpers/hitslop-evaluator"
echo "Embedded hitslop-evaluator in $app/Contents/Helpers"
