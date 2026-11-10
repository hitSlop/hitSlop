# Sharing fixtures: the 2.0.0 baseline

These examples are the pop 2.0.0 sharing contract under the `/v1` routes. They freeze with
the first public release: from then on, add fixtures and never regenerate, replace or delete
existing ones or their expectations. A change an old client cannot read needs a new route
version. Until that release, a contract change rewrites the fixture it changes.

`http.json` holds every public DTO in `src/http.rs` and `src/document.rs`, all phase and role
values, every public ApiFailure code with the statuses that map to it, resource inventories, and extra fields on replies. The HTTP DTO coverage test requires at least one case per published type. Canonical form means JSON value
equality (object key order and whitespace do not matter); all known field names,
omission/null behavior and values must survive.
`invitation.txt` is a pinned-origin invitation link (`/join#<document UUID>.<64 hex>`) that
parses and regenerates byte for byte.
`frames.json` freezes the hand-written frame codec (`hex`) that sockets carry, with one
example per variant, all permissions and all acks, varint boundaries, u64 peers/ids and
UTF-8. Each decodes to the documented `frame` value and re-encodes byte-for-byte. The
fixture bytes are independent expectations, not produced by tests.

`unknown.json` holds fail-closed behavior: unknown JSON phases, frame
tags, permissions and acknowledgements are rejected, and a value missing a required field
(`incomplete`) is refused rather than completed with a default. Unknown HTTP failure codes
remain strings; unknown binary Error codes remain bytes. Extra JSON reply fields are
ignored, including resource inventory fields. Binary trailing bytes are rejected.
