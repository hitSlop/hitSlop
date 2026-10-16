#!/bin/sh
set -eu

if [ "$#" -ne 2 ]; then
  echo "Usage: stapler.sh staple|validate <app-or-dmg>" >&2
  exit 64
fi
case "$1" in
  staple|validate) ;;
  *) echo "Expected staple or validate" >&2; exit 64 ;;
esac

# stapler(1): EX_NOHOST (68) includes unexpected ticket-service responses. Both
# staple and validate fetch tickets. Retry that lookup, never a notary submission;
# invalid/missing/revoked tickets and local write errors still fail immediately.
attempt=1
delay=10
while :; do
  if xcrun stapler "$@"; then
    exit 0
  else
    status=$?
  fi
  if [ "$status" -ne 68 ] || [ "$attempt" -ge 4 ]; then
    exit "$status"
  fi
  echo "stapler $1 ticket lookup failed (68), attempt $attempt/4; retrying in ${delay}s…" >&2
  sleep "$delay"
  attempt=$((attempt + 1))
  delay=$((delay * 2))
done
