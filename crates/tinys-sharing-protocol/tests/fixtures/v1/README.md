# Frozen sharing v1

These examples are immutable once committed. Add fixtures; do not regenerate, replace
or delete existing fixtures or their expectations. Breaking changes need a new path
version while v1 keeps serving old clients.

`http.json` freezes every DTO in `src/http.rs`, all phase values, every current public
ApiFailure code, nullable account lookup, resource inventories, and extra fields on
replies. Canonical form means JSON value equality (object key order and whitespace do
not matter); all known field names, omission/null behavior and values must survive.
`invitation.txt` freezes pinned-origin fragment parsing and link generation.
`frames.json` freezes the hand-written frame codec (`hex`) and the socket's postcard
envelope (`envelopeHex`), with one example per variant, all permissions and all acks,
varint boundaries, u64 peers/ids and UTF-8. Both decode to the documented `frame` value
and re-encode byte-for-byte. The fixture bytes are independent expectations, not
produced by tests.

`unknown.json` freezes fail-closed behavior: unknown JSON phases, control/event tags,
frame tags, permissions and acknowledgements are rejected. Unknown HTTP failure codes
remain strings; unknown binary Error codes remain bytes. Extra JSON reply fields are
ignored, including resource inventory fields. Binary trailing bytes are rejected;
postcard enums must never be reordered or extended on an existing v1 connection.
