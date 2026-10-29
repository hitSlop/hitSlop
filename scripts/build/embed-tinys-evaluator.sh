#!/bin/sh
set -eu

# Named commands need this helper in every app configuration, including Xcode Run.
repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
export PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"
app=${1:?app bundle path required}
profile=${TINYS_CARGO_PROFILE:-release}
(cd "$repo_root" && bun scripts/build/core.ts --engine)
evaluator="$repo_root/target/$profile/tinys-evaluator"
if [ ! -x "$evaluator" ]; then
  echo "tinys-evaluator is missing at $evaluator" >&2
  exit 70
fi
/bin/mkdir -p "$app/Contents/Helpers"
/bin/cp "$evaluator" "$app/Contents/Helpers/tinys-evaluator"
/bin/chmod 755 "$app/Contents/Helpers/tinys-evaluator"
/bin/mkdir -p "$app/Contents/Resources/CaptureFontLicenses"
/bin/cp "$repo_root"/crates/tinys-runner/fonts/*LICENSE* "$repo_root/crates/tinys-runner/fonts/README.md" "$app/Contents/Resources/CaptureFontLicenses/"
/bin/mkdir -p "$app/Contents/Resources/CaptureRendererLicenses"
/bin/cp "$repo_root"/crates/tinys-runner/renderer-licenses/* "$app/Contents/Resources/CaptureRendererLicenses/"
/usr/bin/codesign --force --options runtime --sign "${EXPANDED_CODE_SIGN_IDENTITY:--}" "$app/Contents/Helpers/tinys-evaluator"
echo "Embedded tinys-evaluator in $app/Contents/Helpers"
