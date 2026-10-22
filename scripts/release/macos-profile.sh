#!/bin/sh
# Shared by packaging and local profile-only validation. Never prints profile contents.
profile_error() { echo "macOS provisioning profile: $*" >&2; return 65; }

validate_macos_profile() (
  set -eu
  profile=$1
  required=$2
  work=$3
  [ -f "$profile" ] || { profile_error "file does not exist: $profile"; exit 65; }
  /usr/bin/security cms -D -i "$profile" -o "$work/profile.plist" 2>/dev/null \
    || { profile_error "cannot decode CMS; provide an Apple Developer ID .provisionprofile"; exit 65; }
  /usr/bin/plutil -lint "$work/profile.plist" >/dev/null \
    || { profile_error "decoded payload is not a valid plist"; exit 65; }
  # Older plutil converts the whole profile (dates, certificate data) before extracting, which
  # JSON cannot hold; extract as XML first, then convert only that value.
  json_value() {
    /usr/bin/plutil -extract "$1" xml1 -o "$work/profile-value.plist" "$work/profile.plist"
    /usr/bin/plutil -convert json -o "$2" "$work/profile-value.plist"
  }
  json_value Entitlements "$work/profile-entitlements.json"
  /usr/bin/plutil -convert json -o "$work/required-entitlements.json" "$required"
  expiration=$(/usr/bin/plutil -extract ExpirationDate raw -o - "$work/profile.plist")
  all_devices=$(/usr/bin/plutil -extract ProvisionsAllDevices raw -o - "$work/profile.plist" 2>/dev/null || true)
  json_value Platform "$work/profile-platform.json"
  json_value TeamIdentifier "$work/profile-team.json"
  platform=$(cat "$work/profile-platform.json")
  team=$(cat "$work/profile-team.json")
  bun --eval '
    const [work, expiration, allDevices, platform, team] = process.argv.slice(1);
    const fail = message => { console.error(`macOS provisioning profile: ${message}`); process.exit(65); };
    if (allDevices !== "true" || !JSON.parse(platform).includes("OSX"))
      fail("expected a Developer ID profile for macOS (ProvisionsAllDevices), not development or App Store");
    if (!JSON.parse(team).includes("78UAXU8QG8")) fail("expected team 78UAXU8QG8");
    if (!(Date.parse(expiration) > Date.now())) fail("profile has expired or has an invalid ExpirationDate; regenerate it");
    const allowed = await Bun.file(`${work}/profile-entitlements.json`).json();
    if (allowed["com.apple.application-identifier"] !== "78UAXU8QG8.com.hitslop.app")
      fail("expected App ID 78UAXU8QG8.com.hitslop.app");
    if (allowed["com.apple.developer.team-identifier"] !== "78UAXU8QG8")
      fail("expected entitlement team 78UAXU8QG8");
    if (allowed["get-task-allow"] === true || allowed["com.apple.security.get-task-allow"] === true)
      fail("development debugging is enabled; expected Developer ID distribution");
    const required = await Bun.file(`${work}/required-entitlements.json`).json();
    for (const [key, value] of Object.entries(required)) {
      // App groups, sandbox and hardened-runtime permissions are unrestricted on macOS.
      if (key.startsWith("com.apple.security.")) continue;
      const grant = allowed[key];
      // Apple grants some capabilities as a "*" wildcard (e.g. icloud-services).
      const covers = item => grant === "*" || (Array.isArray(grant) ? grant.includes(item) || grant.includes("*") : grant === item);
      if (!(Array.isArray(value) ? value.every(covers) : covers(value)))
        fail(`Entitlements do not authorize ${key}; regenerate the Developer ID profile with the release capabilities`);
    }
  ' "$work" "$expiration" "$all_devices" "$platform" "$team"
  # ProvisionsAllDevices also exists for enterprise profiles; require a Developer ID certificate.
  /usr/bin/plutil -extract DeveloperCertificates.0 raw -o - "$work/profile.plist" \
    | /usr/bin/base64 -D > "$work/profile-certificate.der"
  subject=$(/usr/bin/openssl x509 -inform DER -in "$work/profile-certificate.der" -noout -subject 2>/dev/null) \
    || { profile_error "cannot read signing certificate"; exit 65; }
  case "$subject" in
    *"Developer ID Application:"*"78UAXU8QG8"*) ;;
    *) profile_error "expected a Developer ID Application certificate for team 78UAXU8QG8"; exit 65 ;;
  esac
)
