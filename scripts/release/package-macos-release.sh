#!/bin/sh
set -eu

# Archive, Developer ID-sign, notarize, and wrap an Apple-silicon hitSlop DMG.
# Required env: MACOS_PROVISIONING_PROFILE (base64) or MACOS_PROVISIONING_PROFILE_FILE (path)
# Optional env:
#   HITSLOP_CODESIGN_IDENTITY   Developer ID identity (name or hash)
#   ASC_API_KEY_P8 / ASC_API_KEY_P8_FILE, ASC_API_KEY_ID, ASC_API_ISSUER_ID
#   SPARKLE_PRIVATE_KEY / SPARKLE_PRIVATE_KEY_FILE
#   SKIP_NOTARIZE=1             skip notarytool (still signs)
#   HITSLOP_SKIP_SYMBOL_UPLOAD=1 skip Crashlytics symbol upload for local hand tests
#   HITSLOP_SKIP_ACCEPTANCE=1   skip host-crash acceptance of the signed app
#   HITSLOP_OUTPUT_DIR          default: dist/macos

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/../.." && pwd)
project_dir="$repo_root/apps/apple"
project="$project_dir/hitSlop.xcodeproj"
project_yml="$project_dir/project.yml"
export_options="$project_dir/App/macOS/ExportOptions.plist"
team_id=${HITSLOP_DEVELOPMENT_TEAM:-78UAXU8QG8}
identity=${HITSLOP_CODESIGN_IDENTITY:-"Developer ID Application: Mushroom DAO Holdings Corp. ($team_id)"}
output_dir=${HITSLOP_OUTPUT_DIR:-"$repo_root/dist/macos"}
sparkle_version=$(bun "$script_dir/notes.ts" --sparkle-version)
feed_prefix=${HITSLOP_DOWNLOAD_URL_PREFIX:-}

# The project-wide build settings own the version the app and its extensions share.
read_macos_setting() {
  /usr/bin/awk -v setting="$1" '
    $0 == "settings:" { in_settings = 1; next }
    in_settings && /^[^ ]/ { in_settings = 0 }
    in_settings && $1 == setting ":" { gsub(/"/, "", $2); print $2; exit }
  ' "$project_yml"
}

marketing_version=$(read_macos_setting MARKETING_VERSION)
build_version=$(read_macos_setting CURRENT_PROJECT_VERSION)
if [ -z "$marketing_version" ] || [ -z "$build_version" ]; then
  echo "Could not read MARKETING_VERSION / CURRENT_PROJECT_VERSION from $project_yml" >&2
  exit 78
fi

bun "$script_dir/notes.ts" --check

if [ -n "${HITSLOP_RELEASE_TAG:-}" ]; then
  expected="v$marketing_version"
  if [ "$HITSLOP_RELEASE_TAG" != "$expected" ]; then
    echo "Release $HITSLOP_RELEASE_TAG does not match MARKETING_VERSION $marketing_version (expected $expected)" >&2
    exit 78
  fi
fi

if [ -z "$feed_prefix" ]; then
  feed_prefix="https://github.com/hitSlop/hitslop/releases/download/v${marketing_version}/"
fi
feed_prefix="${feed_prefix%/}/"

stage_dir=$(/usr/bin/mktemp -d "${TMPDIR:-/tmp}/hitslop-package.XXXXXX")
stage_dir=$(CDPATH= cd -- "$stage_dir" && pwd -P)
lsregister=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
# Builds register with Launch Services as they appear; only an installed app should claim
# .slop, so the exported copy and Xcode's archive copies are unregistered on the way out.
cleanup() {
  "$lsregister" -dump 2>/dev/null \
    | /usr/bin/sed -n 's/^path: *\(.*hitSlop\.app\) (0x[0-9a-f]*)$/\1/p' \
    | /usr/bin/grep -E "^${stage_dir}/|/ArchiveIntermediates/" \
    | while IFS= read -r app; do "$lsregister" -u "$app" >/dev/null 2>&1 || true; done
  if [ -n "${installed_profile:-}" ]; then /bin/rm -f "$installed_profile"; fi
  /bin/chmod -R u+w "$stage_dir" 2>/dev/null || true
  /bin/rm -rf "$stage_dir"
}
trap cleanup EXIT HUP INT TERM

# Validate before looking for identities or building. Keep decoded secrets in the private stage.
. "$script_dir/macos-profile.sh"
profile="$stage_dir/release.provisionprofile"
if [ -n "${MACOS_PROVISIONING_PROFILE_FILE:-}" ]; then
  [ -f "$MACOS_PROVISIONING_PROFILE_FILE" ] || { profile_error "file does not exist: $MACOS_PROVISIONING_PROFILE_FILE"; exit 65; }
  /bin/cp "$MACOS_PROVISIONING_PROFILE_FILE" "$profile"
elif [ -n "${MACOS_PROVISIONING_PROFILE:-}" ]; then
  printf '%s' "$MACOS_PROVISIONING_PROFILE" | /usr/bin/base64 -D > "$profile" 2>/dev/null \
    || { profile_error "MACOS_PROVISIONING_PROFILE is not valid base64"; exit 65; }
else
  profile_error "set MACOS_PROVISIONING_PROFILE (base64) or MACOS_PROVISIONING_PROFILE_FILE (path)"
  exit 65
fi
validate_macos_profile "$profile" "$project_dir/App/macOS/hitSlop.Release.entitlements" "$stage_dir"
HITSLOP_PROVISIONING_PROFILE_UUID=$(/usr/bin/plutil -extract UUID raw -o - "$stage_dir/profile.plist")
export HITSLOP_PROVISIONING_PROFILE_UUID
case "$HITSLOP_PROVISIONING_PROFILE_UUID" in
  ""|*[!A-Za-z0-9-]*) profile_error "invalid UUID"; exit 65 ;;
esac
# Xcode archive/export must be able to resolve the profile too. Preserve existing installations.
profile_dir="$HOME/Library/Developer/Xcode/UserData/Provisioning Profiles"
/bin/mkdir -p "$profile_dir"
profile_install="$profile_dir/$HITSLOP_PROVISIONING_PROFILE_UUID.provisionprofile"
if [ -e "$profile_install" ]; then
  /usr/bin/cmp -s "$profile" "$profile_install" \
    || { profile_error "a different profile with this UUID is already installed"; exit 65; }
else
  installed_profile=$profile_install
  /bin/cp "$profile" "$installed_profile"
fi

# Resolve to a fingerprint: certificates may share a name, but only identities
# reported here have a usable private key. Xcode must select that exact identity.
signing_identities=$(/usr/bin/security find-identity -v -p codesigning)
resolved=$(printf '%s\n' "$signing_identities" \
  | /usr/bin/awk -v identity="$identity" -v team="$team_id" \
      '$0 ~ /Developer ID Application:/ && index($0, "(" team ")") && ($2 == identity || index($0, "\"" identity "\"")) { print $2; exit }')
if [ -z "$resolved" ]; then
  resolved=$(printf '%s\n' "$signing_identities" \
    | /usr/bin/awk -v team="$team_id" '$0 ~ /Developer ID Application:/ && index($0, "(" team ")") { print $2; exit }')
fi
if [ -z "$resolved" ]; then
  echo "No Developer ID Application identity found for team $team_id." >&2
  exit 69
fi
identity=$resolved

# App Store Connect credentials are only for notarization. Archive/export use the
# installed Developer ID identity and must never provision development certificates.
key_path=
if [ -n "${ASC_API_KEY_ID:-}" ] && [ -n "${ASC_API_ISSUER_ID:-}" ]; then
  if [ -n "${ASC_API_KEY_P8_FILE:-}" ]; then
    key_path=$ASC_API_KEY_P8_FILE
  elif [ -n "${ASC_API_KEY_P8:-}" ]; then
    key_path="$stage_dir/AuthKey.p8"
    printf '%s\n' "$ASC_API_KEY_P8" > "$key_path"
  fi
fi

echo "Generating Xcode project…"
/usr/bin/env xcodegen generate --spec "$project_yml" --project "$project_dir"

# Export must use the same validated profile as the main archive target.
/bin/cp "$export_options" "$stage_dir/ExportOptions.plist"
/usr/bin/plutil -insert provisioningProfiles -json "{\"com.hitslop.app\":\"$HITSLOP_PROVISIONING_PROFILE_UUID\"}" "$stage_dir/ExportOptions.plist"
export_options="$stage_dir/ExportOptions.plist"
archive_path="$stage_dir/hitSlop.xcarchive"
export_dir="$stage_dir/export"
/bin/mkdir -p "$export_dir"

echo "Archiving Apple-silicon Release…"
/usr/bin/xcodebuild \
  -skipPackagePluginValidation \
  -quiet \
  -project "$project" \
  -scheme hitSlop-macOS \
  -configuration Release \
  -destination "generic/platform=macOS" \
  -archivePath "$archive_path" \
  -derivedDataPath "$stage_dir/derived" \
  ARCHS="arm64" \
  ONLY_ACTIVE_ARCH=YES \
  HITSLOP_SKIP_SYMBOL_UPLOAD="${HITSLOP_SKIP_SYMBOL_UPLOAD:-0}" \
  HITSLOP_PROVISIONING_PROFILE_UUID="$HITSLOP_PROVISIONING_PROFILE_UUID" \
  DEVELOPMENT_TEAM="$team_id" \
  CODE_SIGN_STYLE=Manual \
  CODE_SIGN_IDENTITY="$identity" \
  archive

echo "Exporting Developer ID app…"
/usr/bin/xcodebuild \
  -quiet \
  -exportArchive \
  -archivePath "$archive_path" \
  -exportPath "$export_dir" \
  -exportOptionsPlist "$export_options"

app="$export_dir/hitSlop.app"
if [ ! -d "$app" ]; then
  echo "Export did not produce hitSlop.app" >&2
  /usr/bin/find "$export_dir" -maxdepth 2 -print >&2
  exit 70
fi

assert_arm64_only() {
  binary=$1
  architectures=$(/usr/bin/lipo -archs "$binary")
  if [ "$architectures" != "arm64" ]; then
    echo "$binary must contain only arm64; found: $architectures" >&2
    exit 70
  fi
}

assert_arm64_only "$app/Contents/MacOS/hitSlop"
assert_arm64_only "$app/Contents/Helpers/hitslop-evaluator"

/bin/cp "$profile" "$app/Contents/embedded.provisionprofile"

echo "Signing nested tools and app…"
/usr/bin/codesign --force --timestamp --options runtime --sign "$identity" "$app/Contents/Helpers/hitslop-evaluator"
/usr/bin/codesign --force --timestamp --options runtime --sign "$identity" "$app/Contents/Frameworks/Sparkle.framework"
/usr/bin/codesign --force --timestamp --options runtime \
  --entitlements "$project_dir/App/macOS/hitSlop.Release.entitlements" \
  --sign "$identity" "$app"
/usr/bin/codesign --verify --deep --strict --verbose=2 "$app"
/usr/bin/codesign -d --entitlements - --xml "$app" > "$stage_dir/signed-entitlements.plist"
validate_macos_profile "$app/Contents/embedded.provisionprofile" "$stage_dir/signed-entitlements.plist" "$stage_dir"
# Assert every requested entitlement survived final signing, including Production CloudKit.
/usr/bin/plutil -convert json -o "$stage_dir/expected-entitlements.json" "$project_dir/App/macOS/hitSlop.Release.entitlements"
bun --eval '
  const [work] = process.argv.slice(1);
  const actual = await Bun.file(`${work}/required-entitlements.json`).json();
  const expected = await Bun.file(`${work}/expected-entitlements.json`).json();
  for (const [key, value] of Object.entries(expected)) {
    if (JSON.stringify(actual[key]) !== JSON.stringify(value)) {
      console.error(`Signed app is missing or has incorrect ${key}`);
      process.exit(65);
    }
  }
' "$stage_dir"

# Notarize only the candidate that passed installed-app and host-crash acceptance.
# Requires the engine from `bun run build` for storage probes.
if [ "${HITSLOP_SKIP_ACCEPTANCE:-}" != "1" ]; then
  echo "Accepting signed app before notarization…"
  (
    cd "$repo_root"
    HITSLOP_APP_BINARY="$app/Contents/MacOS/hitSlop" \
      bun test ./tests/native/crash.native.test.ts
    # Every released document still reads, renders and edits with this build's engine.
    bun test ./tests/native/compat-replay.native.test.ts
  )
fi

versioned="hitSlop-${marketing_version}"
/bin/mkdir -p "$output_dir"
app_zip="$output_dir/${versioned}.zip"
dmg="$output_dir/${versioned}.dmg"
appcast_dir="$stage_dir/appcast"
/bin/mkdir -p "$appcast_dir"

submit_notarization() {
  file=$1
  if [ "${SKIP_NOTARIZE:-}" = "1" ]; then
    echo "Skipping notarization for $file"
    return 0
  fi
  if [ -z "$key_path" ]; then
    echo "ASC_API_KEY_P8, ASC_API_KEY_ID, and ASC_API_ISSUER_ID are required to notarize." >&2
    exit 69
  fi
  echo "Notarizing ${file}…"
  /usr/bin/xcrun notarytool submit "$file" \
    --key "$key_path" \
    --key-id "$ASC_API_KEY_ID" \
    --issuer "$ASC_API_ISSUER_ID" \
    --wait
}

staple() {
  file=$1
  if [ "${SKIP_NOTARIZE:-}" = "1" ]; then
    return 0
  fi
  /bin/sh "$script_dir/stapler.sh" staple "$file"
  /bin/sh "$script_dir/stapler.sh" validate "$file"
}

echo "Zipping app for notarization…"
/usr/bin/ditto -c -k --keepParent "$app" "$app_zip"
submit_notarization "$app_zip"
staple "$app"
/usr/bin/ditto -c -k --keepParent "$app" "$app_zip"

echo "Creating DMG…"
dmg_root="$stage_dir/dmg"
/bin/mkdir -p "$dmg_root"
/usr/bin/ditto "$app" "$dmg_root/hitSlop.app"
/bin/ln -s /Applications "$dmg_root/Applications"
/usr/bin/hdiutil create \
  -volname "hitSlop" \
  -srcfolder "$dmg_root" \
  -ov \
  -format UDZO \
  "$dmg"
/usr/bin/codesign --force --timestamp --sign "$identity" "$dmg"
submit_notarization "$dmg"
staple "$dmg"

# Stable aliases power the landing page's direct download link. Versioned
# artifacts remain available for reproducibility and Sparkle updates.
/bin/cp "$dmg" "$output_dir/hitSlop.dmg"
/bin/cp "$app_zip" "$output_dir/hitSlop.zip"

/bin/cp "$dmg" "$appcast_dir/${versioned}.dmg"
bun "$script_dir/notes.ts" --write "$output_dir"
/bin/cp "$output_dir/${versioned}.md" "$appcast_dir/${versioned}.md"
sparkle_private=${SPARKLE_PRIVATE_KEY_FILE:-}
if [ -z "$sparkle_private" ] && [ -n "${SPARKLE_PRIVATE_KEY:-}" ]; then
  sparkle_private="$stage_dir/sparkle.key"
  printf '%s' "$SPARKLE_PRIVATE_KEY" > "$sparkle_private"
fi
if [ -z "$sparkle_private" ] && [ -f "$repo_root/.secrets/sparkle_eddsa_private.key" ]; then
  sparkle_private="$repo_root/.secrets/sparkle_eddsa_private.key"
fi

echo "Generating Sparkle appcast…"
tools="$stage_dir/sparkle"
/bin/mkdir -p "$tools"
(
  cd "$tools"
  /usr/bin/curl -L --fail --silent --show-error \
    -o Sparkle.tar.xz \
    "https://github.com/sparkle-project/Sparkle/releases/download/${sparkle_version}/Sparkle-${sparkle_version}.tar.xz"
  /usr/bin/tar -xJf Sparkle.tar.xz bin/generate_appcast
)
if [ -n "$sparkle_private" ]; then
  "$tools/bin/generate_appcast" \
    --ed-key-file "$sparkle_private" \
    --download-url-prefix "$feed_prefix" \
    --release-notes-url-prefix "$feed_prefix" \
    -o "$output_dir/appcast.xml" \
    "$appcast_dir"
else
  echo "No Sparkle private key; writing unsigned appcast placeholder skipped." >&2
  echo "Set SPARKLE_PRIVATE_KEY or SPARKLE_PRIVATE_KEY_FILE." >&2
  exit 69
fi

# Retain the exact file processed by Sparkle (including any signing metadata).
/bin/cp "$appcast_dir/${versioned}.md" "$output_dir/${versioned}.md"
notes_url=$(/usr/bin/xmllint --xpath 'string(/rss/channel/item/*[local-name()="releaseNotesLink"])' "$output_dir/appcast.xml")
if [ "$notes_url" != "${feed_prefix}${versioned}.md" ]; then
  echo "Appcast release notes URL differs from the versioned Markdown asset: $notes_url" >&2
  exit 65
fi

(
  cd "$output_dir"
  /usr/bin/shasum -a 256 "${versioned}.dmg" "${versioned}.zip" hitSlop.dmg hitSlop.zip appcast.xml "${versioned}.md" release-notes.md > SHA256SUMS
)

echo "Packaged $marketing_version ($build_version)"
echo "  $dmg"
echo "  $app_zip"
echo "  $output_dir/appcast.xml"
