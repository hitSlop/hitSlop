# Sharing fixtures: the `/v1` routes

These examples are the sharing contract under the `/v1` routes. They freeze with
the first public release: from then on, add fixtures and never regenerate, replace or delete
existing ones or their expectations. A change an old client cannot read needs a new route
version. Until that release, a contract change rewrites the fixture it changes.

`http.json` holds every public DTO in `src/http.rs` and `src/document.rs`, all phase and role
values, every public ApiFailure code with the statuses that map to it, resource inventories, and extra fields on replies. The HTTP DTO coverage test requires at least one case per published type. Canonical form means JSON value
equality (object key order and whitespace do not matter); all known field names,
omission/null behavior and values must survive.
`invitation.txt` is a pinned-origin invitation link (`/join#<document UUID>.<64 hex>`) that
parses and regenerates byte for byte.
`unknown.json` holds fail-closed behavior: unknown JSON phases are rejected, and a value
missing a required field (`incomplete`) is refused rather than completed with a default.
Unknown HTTP failure codes remain strings. Extra JSON reply fields are ignored, including
resource inventory fields. The socket frames' fixtures are in
`crates/pop-contracts/tests/fixtures/relay`.
