#!/bin/sh
set -eu

# Build, embed, and sign the native renderer and Rust document engine in an existing .app.
# Usage: embed-hitslop-native.sh <HitSlop.app>

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/../.." && pwd)
native_package="$repo_root/apps/apple/Packages/HitSlopApple"

app=${1:-}
if [ -z "$app" ] || [ ! -d "$app" ]; then
  echo "Usage: $0 <HitSlop.app>" >&2
  exit 64
fi

scratch=${HITSLOP_NATIVE_SCRATCH:-}
if [ -z "$scratch" ]; then
  scratch=$(/usr/bin/mktemp -d "${TMPDIR:-/tmp}/hitslop-native.XXXXXX")
  trap '/bin/rm -rf "$scratch"' EXIT HUP INT TERM
fi

archs=${HITSLOP_NATIVE_ARCHS:-}
if [ -z "$archs" ]; then
  case "$(/usr/bin/uname -m)" in
    arm64) archs="arm64" ;;
    x86_64) archs="x86_64" ;;
    *) echo "Unsupported macOS architecture: $(/usr/bin/uname -m)" >&2; exit 69 ;;
  esac
fi

arch_flags=
for arch in $archs; do
  case "$arch" in
    arm64|x86_64) arch_flags="$arch_flags --arch $arch" ;;
  esac
done

if [ -z "$arch_flags" ]; then
  echo "No supported architectures were requested: $archs" >&2
  exit 69
fi

configuration=${HITSLOP_NATIVE_CONFIGURATION:-release}
case "$configuration" in debug|release) ;; *) echo "Expected debug or release configuration" >&2; exit 64 ;; esac

echo "Building hitslop-native ($configuration, $archs)…"
# shellcheck disable=SC2086
/usr/bin/swift build \
  --package-path "$native_package" \
  --scratch-path "$scratch" \
  --configuration "$configuration" \
  --product hitslop-native \
  $arch_flags

native_bin=$(
  # shellcheck disable=SC2086
  /usr/bin/swift build \
    --package-path "$native_package" \
    --scratch-path "$scratch" \
    --configuration "$configuration" \
    --product hitslop-native \
    --show-bin-path \
    $arch_flags
)
helper="$native_bin/hitslop-native"
if [ ! -f "$helper" ]; then
  echo "hitslop-native is missing at $helper" >&2
  exit 70
fi

# Build the document engine for exactly the architectures embedded by this host.
# Its core must match the Swift library; checking IDs catches stale generated bindings.
# scripts/build/core.ts owns release builds (their graph, and the flags that keep machine
# paths out of binaries), so the host's engine comes from it, shared with `bun run build`.
export PATH="/opt/homebrew/bin:/usr/local/bin:$HOME/.bun/bin:$PATH"
cargo=${CARGO:-"$HOME/.cargo/bin/cargo"}
# `release` while developing; what ships sets `dist` (scripts/build/core.ts, `cargoProfile`).
profile=${HITSLOP_CARGO_PROFILE:-release}
set --
host_arch=$(/usr/bin/uname -m)
for arch in $archs; do
  if [ "$arch" = "$host_arch" ]; then
    bun "$repo_root/scripts/build/core.ts" --engine
    set -- "$@" "$repo_root/target/$profile/slop-engine"
    continue
  fi
  case "$arch" in
    arm64) target=aarch64-apple-darwin ;;
    x86_64) target=x86_64-apple-darwin ;;
    *) continue ;;
  esac
  RUSTFLAGS=$(bun "$repo_root/scripts/build/core.ts" --rustflags) \
    "$cargo" build --locked --profile "$profile" --manifest-path "$repo_root/Cargo.toml" \
    --target-dir "$repo_root/target" --target "$target" -p slop-engine
  set -- "$@" "$repo_root/target/$target/$profile/slop-engine"
done
engine="$scratch/slop-engine"
/usr/bin/lipo -create "$@" -output "$engine"
if [ "$("$engine" --build-id)" != "$("$helper" --core-build)" ]; then
  echo "slop-engine and hitslop-native embed different cores; rebuild native bindings" >&2
  exit 70
fi

# SwiftPM executables locate Bundle.module resources beside the executable.
# Host resources are not visible to this independently-built helper.
/bin/mkdir -p "$app/Contents/Helpers"
/bin/cp "$helper" "$app/Contents/Helpers/hitslop-native"
/bin/chmod 755 "$app/Contents/Helpers/hitslop-native"
/bin/cp "$engine" "$app/Contents/Helpers/slop-engine"
/bin/chmod 755 "$app/Contents/Helpers/slop-engine"

# Xcode does not automatically sign nested content added by a run script. Use
# its resolved identity for normal builds and an ad-hoc signature when signing
# is disabled; distribution packaging replaces both signatures as needed.
signing_identity=${EXPANDED_CODE_SIGN_IDENTITY:--}
for module in HitSlopDocument; do
  resource_bundle="$native_bin/HitSlopApple_${module}.bundle"
  if [ ! -d "$resource_bundle" ]; then
    echo "hitslop-native resource bundle is missing at $resource_bundle" >&2
    exit 70
  fi
  embedded="$app/Contents/Helpers/HitSlopApple_${module}.bundle"
  /usr/bin/ditto "$resource_bundle" "$embedded"
  # SwiftPM's Xcode build system emits macOS Contents bundles; its native
  # build system emits flat resource bundles. Preserve the emitted layout.
  info="$embedded/Info.plist"
  if [ -d "$embedded/Contents" ]; then info="$embedded/Contents/Info.plist"; fi
  /bin/cp "$script_dir/HitSlopNativeResources-Info.plist" "$info"
  /usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier com.hitslop.app.native-resources.$module" "$info"
  /usr/libexec/PlistBuddy -c "Set :CFBundleName $module Resources" "$info"
  /usr/bin/codesign --force --sign "$signing_identity" "$embedded"
done
/usr/bin/codesign --force --options runtime --sign "$signing_identity" "$app/Contents/Helpers/hitslop-native"
/usr/bin/codesign --force --options runtime --sign "$signing_identity" "$app/Contents/Helpers/slop-engine"
echo "Embedded hitslop-native and slop-engine in $app/Contents/Helpers"
