#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd)
project="$repo_root/apps/apple/hitSlop.xcodeproj"
destination=${HITSLOP_APP_DESTINATION:-/Applications/hitSlop.app}
team_id=${HITSLOP_DEVELOPMENT_TEAM:-78UAXU8QG8}
signing_identity=${HITSLOP_DEVELOPMENT_IDENTITY:-Apple Development}
build_arch=$(/usr/bin/uname -m)
stage_dir=$(/usr/bin/mktemp -d "${TMPDIR:-/tmp}/hitslop-release.XXXXXX")
install_stage="$(dirname -- "$destination")/.hitSlop.installing.$$"
backup="$stage_dir/previous-hitSlop.app"
lsregister=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister

# Bundled starter templates are read-only, so each removal restores write access first.
remove() { /bin/chmod -R u+w "$1" 2>/dev/null; /bin/rm -rf "$1"; }
cleanup() {
  remove "$stage_dir"
  if [ -e "$install_stage" ]; then remove "$install_stage"; fi
}
trap cleanup EXIT HUP INT TERM

case "$build_arch" in
  arm64|x86_64) ;;
  *) echo "Unsupported macOS architecture: $build_arch" >&2; exit 69 ;;
esac

echo "Building development-signed hitSlop Release for ${build_arch}…"
/usr/bin/env xcodegen generate --spec "$repo_root/apps/apple/project.yml" --quiet
/usr/bin/xcodebuild \
  -skipPackagePluginValidation \
  -skipMacroValidation \
  -quiet \
  -allowProvisioningUpdates \
  -project "$project" \
  -scheme hitSlop-macOS \
  -configuration Release \
  -derivedDataPath "$stage_dir/derived" \
  DEVELOPMENT_TEAM="$team_id" \
  CODE_SIGN_STYLE=Automatic \
  CODE_SIGN_IDENTITY="$signing_identity" \
  ARCHS="$build_arch" \
  ONLY_ACTIVE_ARCH=YES \
  build

app="$stage_dir/derived/Build/Products/Release/hitSlop.app"

for required in "$app" "$app/Contents/Helpers/HitSlopApple_HitSlopDocument.bundle"; do
  if [ ! -e "$required" ]; then
    echo "Release output is missing: $required" >&2
    exit 70
  fi
done

/usr/bin/codesign --verify --deep --strict --verbose=2 "$app"

echo "Installing ${destination}…"
if [ -e "$install_stage" ]; then remove "$install_stage"; fi
/usr/bin/ditto "$app" "$install_stage"
if [ -e "$destination" ]; then "$lsregister" -u "$destination" >/dev/null 2>&1 || true; fi
if [ -e "$destination" ]; then /bin/mv "$destination" "$backup"; fi
if ! /bin/mv "$install_stage" "$destination"; then
  if [ -e "$backup" ]; then /bin/mv "$backup" "$destination"; fi
  echo "Could not replace $destination; restored the previous app." >&2
  exit 73
fi
if ! /usr/bin/codesign --verify --deep --strict --verbose=2 "$destination"; then
  remove "$destination"
  if [ -e "$backup" ]; then /bin/mv "$backup" "$destination"; fi
  echo "Installed app failed verification; restored the previous app." >&2
  exit 74
fi

"$lsregister" -f "$destination"
# Launch Services alone leaves the replaced app's Quick Look extensions unregistered, so
# Finder would show the generic .slop icon until something else registered them.
for extension in "$destination"/Contents/PlugIns/*.appex; do /usr/bin/pluginkit -a "$extension" || true; done
/usr/bin/qlmanage -r >/dev/null 2>&1 || true
/usr/bin/qlmanage -r cache >/dev/null 2>&1 || true
/usr/bin/killall Finder >/dev/null 2>&1 || true

echo "Installed hitSlop $(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$destination/Contents/Info.plist") ($(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$destination/Contents/Info.plist"))"
echo "Bundled helper: $destination/Contents/Helpers/hitslop-native"
