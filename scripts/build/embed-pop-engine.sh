#!/bin/sh
set -eu

# Named commands need this helper in every app configuration, including Xcode Run.
repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
export PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"
app=${1:?app bundle path required}
profile=${POP_CARGO_PROFILE:-release}
(cd "$repo_root" && bun scripts/build/core.ts --engine)
engine="$repo_root/target/$profile/pop-engine"
if [ ! -x "$engine" ]; then
  echo "pop-engine is missing at $engine" >&2
  exit 70
fi
# Replace owned build output so incremental app builds also ship only one helper.
/bin/rm -rf "$app/Contents/Helpers"
/bin/mkdir -p "$app/Contents/Helpers"
/bin/cp "$engine" "$app/Contents/Helpers/pop-engine"
/bin/chmod 755 "$app/Contents/Helpers/pop-engine"
/bin/mkdir -p "$app/Contents/Resources/CaptureFontLicenses"
/bin/cp "$repo_root"/crates/pop-runner/fonts/*LICENSE* "$repo_root/crates/pop-runner/fonts/README.md" "$app/Contents/Resources/CaptureFontLicenses/"
/bin/mkdir -p "$app/Contents/Resources/CaptureRendererLicenses"
/bin/cp "$repo_root"/crates/pop-runner/renderer-licenses/* "$app/Contents/Resources/CaptureRendererLicenses/"
/usr/bin/codesign --force --options runtime --sign "${EXPANDED_CODE_SIGN_IDENTITY:--}" "$app/Contents/Helpers/pop-engine"
echo "Embedded pop-engine in $app/Contents/Helpers"
