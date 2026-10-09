#!/usr/bin/env bash
# Keep local and CI browser optimizers identical; distribution packages may be too old.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
destination="$root/generated/core-tools/binaryen"
version=132
check_version() {
  local reported
  reported="$("$1" --version 2>/dev/null)" || return 1
  [[ "$reported" == "wasm-opt version $version" || "$reported" == "wasm-opt version $version (version_$version)" ]]
}
if check_version "$destination/bin/wasm-opt"; then
  "$destination/bin/wasm-opt" --version
  exit 0
fi

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)
    platform=x86_64-linux
    digest=195ddc94f9bc89f45abdabb0b9eea86023d727ba90eac8b35b80f2544fc30572 ;;
  Linux-aarch64|Linux-arm64)
    platform=aarch64-linux
    digest=c58562417836c5d0493d89bdefc434933bdc097db641b483df86bcfa557a107f ;;
  Darwin-arm64)
    platform=arm64-macos
    digest=98aad827847af7ef990ed7098d885725c8e5b5aae75073403635617ae4e259aa ;;
  Darwin-x86_64)
    platform=x86_64-macos
    digest=40c3de90bb3766bd0282a895e139a6f50253dba49b4f5bb89e66faca162d832e ;;
  *) echo "Binaryen installer supports Linux and macOS on x86_64 or arm64" >&2; exit 1 ;;
esac

mkdir -p "$root/generated/core-tools"
stage="$(mktemp -d "$root/generated/core-tools/.binaryen-XXXXXX")"
trap 'rm -rf "$stage"' EXIT
archive="$stage/binaryen.tar.gz"
curl --fail --location --retry 3 \
  "https://github.com/WebAssembly/binaryen/releases/download/version_$version/binaryen-version_$version-$platform.tar.gz" \
  --output "$archive"
if command -v sha256sum >/dev/null; then
  echo "$digest  $archive" | sha256sum --check -
else
  echo "$digest  $archive" | shasum -a 256 --check -
fi
tar -xzf "$archive" -C "$stage"
installed="$stage/binaryen-version_$version"
check_version "$installed/bin/wasm-opt"
rm -rf "$destination"
mv "$installed" "$destination"
"$destination/bin/wasm-opt" --version
