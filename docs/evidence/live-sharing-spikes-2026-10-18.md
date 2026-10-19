# Rust Cloudflare sync and Loro protocol review — 2026-10-18

## Result and recommendation

The Rust Cloudflare adapter works locally and on Cloudflare. Four transport variants
passed the current core's local convergence/retry scenarios; the preferred opaque
variant also passed a gated disposable hosted probe. These do not certify the three-host UI.

Prefer an **opaque, version-vector-indexed byte relay**, retaining the existing owner's
acceptance/publication/save path and reconnect reconstruction. Replace the handwritten
frame encoding with a small Rust-owned serde/postcard envelope. One update per push is
enough for the current Session and avoids unnecessary multi-record push semantics.

Do not adopt the upstream Rust client/server unchanged. Do not put a Loro metadata
decoder on the server merely to remove span headers: it adds substantial code and does
not establish complete or valid history. Full-log replay is useful as a baseline, but
adds unnecessary repeated traffic.

This is a recommendation from an isolated experiment, not a production wire commitment.
The wire/platform spike changed no production sync code, Loro pin, document format or
frozen compatibility fixture. The later library-boundary follow-up is recorded below.

Postcard is an encoding choice, not a compatibility policy. Derived enum indices and
field order are wire details; adding a field does not automatically preserve decoding.
Before freezing a public protocol, give the envelope explicit version dispatch and retain
every released decoder, or choose an extensible field encoding. The alternate-wire bridge
used here is a test adapter; production types must have one Rust owner.

## Sources and provenance

The user-supplied trees were reviewed directly:

- `_docs/loro/protocol-main`: protocol.md, protocol-e2ee.md, the Rust codec/client/server,
  TypeScript client and document adaptors, and the corresponding tests.
- `_docs/loro-main`: public Loro API, import/export/metadata implementation, shallow
  snapshot semantics, pending changes, failed-import rollback and peer-id reuse.

Five relevant Loro implementation files match the product's exact git pin
`c00c9fa501f8d32f68d6255eacb7035a67fb6ab6` byte for byte: public lib.rs, internal
loro.rs, encoding.rs, encoding/fast_snapshot.rs and oplog/pending_changes.rs.
All five Rust codec source files in the local protocol tree match the published
`loro-protocol = 0.3.0` used by the executable probes. These are source comparisons,
not a claim that a directory named main is a newer release.

Root HEAD during the review: `67bf4de4a83ab8bb6fdf2e4baf2cf7f2222e188c`, with the
user's staged/unstaged iOS and library changes present. Core source hashes:

| File | SHA-256 |
|---|---|
| sync/mod.rs | 756a042915193b94cae9ddc96b658b34def66ec33274c742a337aebcb5775cab |
| sync/frame.rs | 4f504168274010d0c4ea2a497398f7a7b1cb17e72f4c01734aefaa372ed8125f |
| document/mod.rs | 6f8ca8984cee48adadd86b38914b4c9aebf098543597f837bc1f810899391e38 |

The isolated source, Cargo.lock, machine-readable audit and run artifacts live under
`spikes/live-sharing-rust/`. Both spikes/ and plans/ are intentionally Git-ignored;
this tracked evidence page does not imply those local sources exist in a fresh clone.

## What the upstream protocol gives us

The transport-independent codec supports joining, permissions, acknowledgments,
room errors, update batches, fragmentation, multiple rooms and multiple CRDT kinds.
The Rust codec has no external dependencies. A 3,000,000-byte payload round-tripped
through thirteen fragments, occupying 3,000,421 encoded bytes. A single message of that
size is refused by the encoder.

These are useful reference designs. The 256 KiB message ceiling is an upstream protocol
choice; its documentation's older Cloudflare limit is not the current platform limit.
Our existing one-room-per-socket design does not need multiplexing, other CRDTs or
the ELO encryption extension to deliver the first sharing slice.

### The upstream reference Rust server is not a durable relay

Source: `rust/loro-websocket-server/src/lib.rs` in the supplied protocol tree.

- `LoroRoomDoc::apply_updates` discards import errors and returns success. It also
  ignores loading errors in `import_snapshot`. An acknowledged update is therefore
  not evidence of successful document import.
- The DocUpdate handler broadcasts and returns a success Ack after in-memory application.
  Saving is a separate optional interval task. A server crash can precede persistence
  of an acknowledged update. This is incompatible with hitSlop's meaning of Synced.
- Join attempts to send the complete initial snapshot as one DocUpdate. If the snapshot
  exceeds the codec's message ceiling, encoding failure is silently skipped there.
  Incoming fragmentation support does not automatically make bootstrap support large files.
- Fragment buffers live in memory. Fragments are broadcast before complete reassembly
  and application. This does not provide a durable, hibernation-safe batch lifecycle.
- The plaintext room's default version/backfill methods do not implement the efficient
  delta negotiation our current relay uses; joins favor whole snapshots.

The generic protocol can carry stronger semantics, but the reference server does not
supply those semantics for us. These findings are code inspection, not an executed
upstream-server crash campaign.

### Client integration still needs our owner

The Rust `join_loro` and standard Loro adaptor hold and import into their own LoroDoc.
Using them directly would bypass the hitSlop owner, app acceptance, publications and
persistence ordering. A custom adaptor is possible, but would still need those boundaries.

The Rust implementation's socket reader exits on disconnect; it does not provide the
TypeScript client's reconnect supervisor. The TypeScript client has backoff/rejoin
machinery and useful stale-socket fences. Its joined/connected states must not be
presented as hitSlop's durable Synced state.

The standard Rust adaptor ignores permission in its join callback and discards import
errors. The TypeScript adaptor reports import failures more carefully, but both are
examples of integration machinery rather than a replacement for our owner.
Suppressing network sends is not enforcement of local read-only behavior.

### The upstream codec also needs hardening

Executable characterization against published 0.3.0, whose sources match the supplied tree:

- Decoder accepts a valid message with trailing bytes, including an input larger than
  the encoder's own message ceiling.
- A tiny DocUpdate declaring count=u64::MAX panics on checked vector capacity overflow.
  The probe catches the panic before allocation; it does not allocate a huge buffer.
- Its varint reader accepts an overflowing tenth byte and truncates the value.

These are prototype findings, not production exploit claims. They rule out treating
upstream adoption as a way to eliminate boundary validation without doing further work.
The postcard candidate refuses the targeted malformed-length/trailing-byte cases.

## Loro behavior that the relay must respect

Source: the supplied Loro implementation, confirmed against the product pin.

### Metadata inspection is not a proof of a complete update

`decode_updates_blob_meta` builds temporary Loro/oplog structures and decodes the changes.
It is not a cheap fixed-header read. Its partial version vectors take minimum and maximum
counters for each peer.

An executable probe exports counters [0,2) and [4,6) for one peer into a checksum-valid
update. Metadata reports a range from 0 to 6. Import applies the first part and leaves
the later part pending. Thus replacing trusted span headers with these min/max vectors
does not establish contiguous history. Import acceptance and causal checks remain necessary.

The FastUpdates checksum is XXH32. It detects corruption; it is not authentication or
app-state validation. Neither the opaque nor metadata-inspecting experiment protects
a room from an authorized writer deliberately publishing unusable updates. Client
refusal must preserve local files, and room-ending/independent-copy recovery remains
necessary before public launch.

### Pending changes are not a durable inbox

A checksum-valid update can import successfully while waiting for a missing dependency.
A snapshot exported at that point does not preserve those pending bytes in the probe.
After reopening, the dependency alone is insufficient; the pending update must be delivered
again. The current reconnect handshake asks from the integrated version and can recover it.

Retain missing-history detection, replayable durable relay records and an honest caught-up
barrier. Do not equate successful transport delivery, successful import, locally saved,
relay stored and iCloud confirmed.

### Full history and shallow roots

A shallow seed rejects edits based before its root. A full snapshot imported later does
not generally turn a shallow replica into a full-history replica. Server compaction cannot
silently replace retained history with a new shallow root.

A stale, same-UUID document can also lack changes already included in the room seed.
The current Session pauses after CaughtUp if the relay claims those seeded changes but
only replays post-seed records. Merging the seed through the existing document first
preserves concurrent local edits and resolves that failure.

The product's current full-history retention rule is therefore still justified.
The final full-file join must validate UUID/app digest and merge through the existing
owner, not overwrite its file.

The socket experiment drives Session/Document, not a real owner evaluating commands.
The current owner's held-frame queue is bounded by frame count (256), not aggregate
bytes. Native integration must bound queued bytes and apply backpressure before treating
the protocol's large-frame ceiling as a safe operating size.

### Commit boundaries and rollback

Keep whole committed update exports. Slicing arbitrary operation spans can expose an
incomplete row or missing dependencies even though the underlying bytes decode.
The owner must retain its application-level acceptance and rollback behavior; a library's
generic Loro adaptor does not provide it. Existing per-session random peers and immutable
replica identity rules remain important because version vectors are not content hashes.

## Rust Cloudflare experiment

Tooling: worker/worker-build 0.8.7, Wrangler 4.149.0, Rust 1.96.1,
wasm-bindgen 0.2.129, postcard 1.1.3, and the exact product Loro pin.

The Worker uses DO SQL storage with sub-row-size blob chunks, hibernating WebSocket
acceptance/socket attachments, R2 streaming with SHA-256 upload checking, a Rate Limiting
binding, credential hashes, a ready-after-seed state and ended-room tombstones.
workers-rs exposes SQL but lacks its transactionSync wrapper. One small wasm-bindgen
binding to that platform method works; a TypeScript relay rewrite was unnecessary.

The baseline compiles the current frame source directly. Experimental adapters drive the
real current Session/Document over the alternate transports without another client engine.

### Local socket results

All four passed: online edits, concurrent offline edits, duplicate acknowledged pushes,
a stored push whose acknowledgment the sender did not consume, runtime process restart
with persistent SQLite/R2, 1,000 offline edits, R2-seeded late join, a low-compressibility
3 MB edit crossing the SQL row boundary, wrong app, credential refusals and ending the room.

| Transport | 1,000-edit catch-up | 3 MB edit | Warm reconnect received |
|---|---:|---:|---:|
| Existing binary | 67 ms | 364 ms | 33 B |
| Serde/postcard, opaque | 80 ms | 362 ms | 38 B |
| Text controls/raw updates, inspected metadata | 101 ms | 453 ms | 119 B |
| Full replay baseline | 484 ms | 557 ms | 140,081 B |

Local rerun after extracting the common relay fixture, including automatic ping/pong:
27.61 seconds (`out/network.json`). Not a statistical
benchmark. Batching differs by scheduling, and the
experimental alternate-client bridge performs extra conversions. Do not infer production
latency ratios from this table. The full-replay variant retains the same write index;
it isolates replay traffic, not a complete independent sequence-cursor implementation.

Matched initial feature builds measured 591.67 KiB raw / 177.12 KiB gzip without Loro
inspection, versus 2008.49 KiB raw / 591.52 KiB gzip with it. The later preferred build,
including postcard and hosted-probe gating, is 618.00 KiB raw / 185.95 KiB gzip.
Builds skipped wasm-opt; these are deploy-bundle sizes, not memory measurements.

R2 bootstrap in the protocol harness is a core Loro checkpoint, not a complete .slop
installer. The owner follow-up below qualifies local full-file installation and seed
attachment retention; new-attachment transport and native UI integration remain separate gates.

### Hosted probe

The opaque serde/postcard build deployed successfully: 618.00 KiB raw / 185.95 KiB gzip;
Cloudflare reported 15 ms Worker startup. This is a deployment metric, not measured
end-to-end cold-open latency.

The current-core native test client passed against actual HTTPS/WebSockets, DO SQL and
R2: online/offline edits, duplicate pushes, a sender losing an acknowledgment, 1,000-edit
catch-up, seed fetch, late join, a 3 MB update, wrong app/credentials, and room ending.
Catch-up measured 769 ms; the 3 MB edit measured 1,886 ms; warm reconnect received 39 B.
The run completed in 15.01 seconds. These are single-run workstation measurements,
not iPhone/browser measurements or a latency service-level claim.

Remote mode deliberately skips the local process-restart operation and records that
fact in out/network-hosted.json. It does not qualify regional outages or actual
hibernation eviction/restoration.

The disposable Worker was hitslop-sync-spike-3b04bb3f, deployment
a0133428-aa54-4f7d-bfd4-3cf20e2cbfb3. Its Worker and empty R2 bucket were deleted with
Wrangler after the synthetic room was ended. No landing routes, production data or
Apple app capabilities changed. The secret gate was never logged or put in a URL.

## Real owner/library coexistence follow-up

The next local spike passed through the actual `Library`, `Owner`, SQLite store and
owner sync hooks, using the preferred serde transport and the existing Rust DO under
local workerd. Two managed library replicas and a late standalone replica converged.
`out/owners.json` records the result; `tests/owners.rs` is the isolated harness.

This uses complete `.slop` seeds in R2, unlike the earlier protocol fixture's raw Loro
checkpoint. The measured seed was 49,152 bytes. A stale same-UUID owner first paused
with missing history when connected without merging the seed. Validating and merging
the full seed through that same owner preserved its unsent edits and brought across
the immutable seed attachment. A third owner installed that full seed into a new local
file and caught up without creating a CloudKit library or account.

The completed scenarios check:

- Reordered/duplicate checkpoint downloads interleaved with relay traffic, online and
  offline concurrent edits, and contiguous visible publication sequence numbers.
- An acknowledged duplicate produces neither a new relay record nor a publication;
  a stored push whose Ack is not consumed converges after reconnect.
- Closing/reopening actual owners and restarting the local relay process retains
  UUIDs, original paths/inodes, history, relay bindings and seed attachment bytes.
- Old upload receipts do not hide merged edits. A wrong seed UUID is refused before
  import. Reopening a library under another account is refused, and an unregistered
  live owner's writer lease prevents library staging until that owner closes.
- Local editing and whole-file checkpoint merging work with sockets disconnected and
  after sharing ends. Prototype binding policy detects a conflicting room instead of
  silently replacing it; this policy is not yet a production credential API.

A regression at the owning library boundary reproduced the actual membership overwrite:
`snapshot_delivery_preserves_the_live_owners_relay_binding` failed because a download
replaced the relay row with `cloudkit://library`. The receive boundary now initializes the
retention placeholder only when no share row exists. The regression then passed for both live and
closed-owner receives; all ten library boundary tests passed. Snapshot history imports
continue to retain each existing owner's local binding.

A virtual clock drove 25 edits at five-second intervals over 120 seconds. A prototype
gate checked both simulated enqueue and automatic-send callbacks, allowing at most one
automatic staged attempt per 60 seconds per replica. The first completed run produced
five automatic uploads across two replicas, then none during idle checks at 180, 240
and 300 seconds. Explicit Sync Now at 121 seconds bypassed automatic eligibility.
Staged files including that explicit upload totaled 335,872 bytes in this run. These
are fixture measurements, not a production traffic estimate. Eligibility reads the
saved version; the virtual clock explicitly advances the owner save boundary before
testing scheduling, matching the native library's saved-state reads.

The socket adapter has bounded aggregate queued bytes and frame count, and admits
inbound frames one at a time with an owner mailbox fence. The owner's separate
command-held queue still lacks an aggregate byte budget; commands are not qualified by
this harness. Reconnects are explicitly driven by the harness rather than a shipping
connection supervisor.

CloudKit delivery, scheduling and trusted credential discovery are simulated here.
The current foreground editor calls `transport.sync()` every 15 seconds, and that
method also calls `sendOnly()`. Actual integration must distinguish automatic refresh
from user Sync Now so the refresh cannot use the explicit throttle bypass. Changing
the one-second idle timer alone would not enforce the prototype policy.
No signed Keychain access, CKSyncEngine enqueue/send wiring, physical iPhone lifecycle,
browser-origin isolation, new attachment transport or real hibernation eviction is
implied. No new Worker deployment was needed for this follow-up.

Follow-up checks: all twelve regular spike probes passed, native spike clippy passed
with warnings denied, all four socket variants passed again in 27.61 seconds after
extracting their common local-relay fixture, and the owner coexistence harness passed
again in 7.90 seconds. `out/owner-source-hashes.json` records the current source/build
hashes. The owning library regression failed before the preservation fix and all ten
library tests passed afterward. The first broad repository verification attempt was
interrupted during the portable authoring check, and the attempted filtered retry
encountered another process's active checkout lease; it did not bypass or remove it.
That shared `bun run verify --native` run subsequently completed successfully, including
all 419 selected Rust tests, contracts, Bun, CLI, render, packed, Swift and native tiers.
Its retained report is
`.hitslop/evidence/runs/2026-10-18T23-25-45-173Z-70543/report.json`.
A final `bun run verify` exited successfully, confirming every affected tier had already
passed with these exact inputs. `out/owner-verification.json` records that final result.
Native verification here covers repository fixtures, not signed cross-device sharing.

## Verification and next work

- Local boundary probes: current seed behavior, codec anomalies, real Loro pending/trim
  behavior, upstream fragmentation and decoder anomalies.
- Socket harness: the current core over all four experimental transports.
- Worker dry-run artifacts for opaque and metadata-inspecting builds.
- `bun run verify` passed in 600.0 s, including all 417 selected Rust tests. Its report is
  `.hitslop/evidence/verify.json`. This includes the user's current worktree; it is not
  proof of a shipping sharing feature.
- No Swift/FFI implementation changed, so this spike did not run the native/device matrix.
- Native and WASM spike clippy passed with warnings denied. A later repository verify
  attempt encountered another process's active checkout lease; it did not bypass or
  remove that lease. The completed 600.0-second run above remains the recorded run.

The local real-owner coexistence proof above is now complete. Retain its checkpoint
policy and wire as experimental. Next prove signed Mac/iPhone Keychain delivery and
actual CKSyncEngine scheduling, then qualify hosted browser origins; deliver
new attachments and history-full recovery; finally exercise signed Mac/iPhone and hosted
Chrome together. Real hibernation eviction/restoration, public-network failure campaigns,
per-room quota behavior, sustained memory/backpressure and concurrent room creation
remain unqualified unless separately recorded.

## Reproduction

See `spikes/live-sharing-rust/README.md`. Artifact files include `out/network.json`,
`out/source-audit.json`, Worker build outputs, isolated Wrangler logs and the Cargo lock.
Expected panic text in the upstream malformed-count probe is caught by the test.

Reference documentation:
- https://github.com/loro-dev/protocol
- https://github.com/loro-dev/loro/tree/c00c9fa501f8d32f68d6255eacb7035a67fb6ab6
- https://developers.cloudflare.com/durable-objects/best-practices/websockets/
- https://developers.cloudflare.com/durable-objects/platform/limits/
