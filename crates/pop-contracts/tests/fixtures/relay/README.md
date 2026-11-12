# Relay frame fixtures

These examples are the pop 2.0.0 relay frame codec. They freeze with the first public
release: from then on, add fixtures and never regenerate, replace or delete existing ones or
their expectations. Until that release, a contract change rewrites the fixture it changes.

`frames.json` freezes the hand-written frame codec (`hex`) that sockets carry, with one
example per variant, all permissions and all acks, varint boundaries, u64 peers/ids and
UTF-8. Each decodes to the documented `frame` value and re-encodes byte-for-byte. The
fixture bytes are independent expectations, not produced by tests.

`unknown.json` holds fail-closed behavior: unknown frame tags, permissions and
acknowledgements are rejected, and unknown binary Error codes remain bytes. Binary trailing
bytes are rejected.
