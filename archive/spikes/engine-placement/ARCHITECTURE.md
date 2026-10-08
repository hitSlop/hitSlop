# Proposed boundary if native ownership proceeds

This is a design outcome of the spike, not an implemented platform cutover. Mac is
the only executable target here. There is no browser editing requirement.

## One portable semantic engine

Loro is Rust in both candidates. What must also become portable is hitSlop's
interpretation of descriptors: identity, strict authored writes, preserve-and-flag
reads, projections, transactions, undo boundaries and text anchors. Sharing only
Loro while separately implementing those rules in Swift and a Tauri backend would
repeat the expensive part.

If native ownership is selected, prefer a **shared Rust document core** with a
small Swift binding and a Tauri adapter. The Swift binding prototype tests feasibility
and costs; it should not automatically become the permanent implementation language
for all document semantics. Its native API coverage is intentionally incomplete.

The proposed responsibilities are:

| Component | Owns |
|---|---|
| Shared document core | One Loro replica, descriptor interpretation, validation, identity resolution, atomic batches, projection patches, text anchors, accepted and durable versions |
| Host persistence adapter | Writer ownership, safe package paths, SQLite transactions, attachment blobs, generation recovery and filesystem errors |
| Host sync service | Room binding and credentials, connection lifecycle, durable outbound ranges, incoming bytes and acknowledgements |
| WebView SDK | Typed intent construction, immutable subscribed views, temporary input drafts, selection/composition and acceptance promises |
| Platform UI | Windows, lifecycle, pickers, export, native save/error/retry UI and document discovery |

Keep the descriptor and platform contract as data. Generate bridge and binding types
from their source definitions. A Swift `JSONValue`/Rust `serde_json::Value` at the
application-data boundary is appropriate; untyped dictionaries throughout the engine
are not. Arbitrary slop schemas do not require generating a Swift app model.

These are responsibility boundaries, not a requirement for five packages. Keep the
SQLite transaction/generation/recovery policy shared where practical; platform
adapters should provide paths, ownership primitives and lifecycle signals. Avoid
copying save semantics into separate Swift and Tauri implementations merely because
their UI frameworks differ.

## Public API freedom

The current DSL, `change()` shape, snapshots and handles are not migration constraints.
Design around intent, not sending modified application JSON. A synchronous callback
can construct a batch, while awaiting its submission means the host accepted it.
`flush()` means local persistence completed; neither promise means server durability.

Use application IDs and stable text positions for edits based on an older frame.
The spike's positional move envelope is safe only in its serialized local benchmark;
it must not become the collaboration API. A production batch must carry a base
revision and stable identities/anchors, with explicit rejection for stale intent that
cannot be interpreted. Never silently retarget an operation to a new row at an old
index.

Text bindings are part of the SDK, not a task delegated to every slop author. They
must retain draft ancestry, handle remote deletion, preserve IME composition and
selection, and avoid resubmitting acknowledged text. A local draft is temporary input,
not a second CRDT replica. Real keyboard/IME tests remain required.

Subscriptions should be able to request the fields or list ranges the renderer needs.
The current spike sends a complete initial view; that cost is visible in opening
measurements. Do not introduce a full persistent JSON mirror to avoid it.

## Durability and synchronization

Preserve the single-writer rule and incremental SQLite storage. A native owner does
not make accepted edits durable by itself. Failed saves retain work and ownership;
close/export wait for drafts and durable storage. Track accepted, durable and
server-acknowledged progress explicitly.

Only durable changes may enter the outbound stream. Decide the protocol using a
correctness harness before removing an outbox or declaring a version vector sufficient.
An opaque ordered relay and a merge-aware version-vector server have different
requirements; this spike's echo diagnostic proves neither. Missing dependencies,
reordered messages, duplicate delivery, lost replies, server restart and reconnect
must have end-to-end ownership tests. Receive progress cannot advance past unsaved
imports. A sync binding is host-owned and distinct from copied package identity.

Keep peer identity separate from user identity, room identity and copied document
identity. Reusing a peer requires restoring its durable operation history before
editing and fencing concurrent writers. A device-plus-document-ID key alone does
not handle Finder copies or restoring an older package. Fresh session IDs remain
the safe default until that lifecycle is proved.

Engine placement does not change the Loro wire format: native and WASM replicas can
exchange compatible updates, as the component replay demonstrates. An opaque relay
can store and forward those bytes without understanding the document. Producing a
new compacted checkpoint is a different responsibility: either the server runs a
compatible Loro engine, or a separately designed client-checkpoint protocol supplies
one. Do not describe deleting old log entries as safe compaction without that proof.
Cloudflare deployment, limits and authentication are outside this local experiment.

Loro 1.16.2 still reproduces the counter regrouping failure: accepted 1, replayed 0,
same version vector. Local checkpoints preserve reopen behavior but do not prove
counter convergence between peers. Before collaboration, either obtain an upstream
fix with regression evidence or choose a different precisely specified counter model.
Do not assume switching to Swift, restricting individual inputs to safe integers, or
using a different transport solves it.

## Later iOS work

- Reuse the same core and persistence contracts; isolate AppKit, CLI sockets, desktop
  pickers and window behavior behind platform adapters.
- Store writable documents in app-owned local storage. Do not open a live SQLite
  writer in a File Provider or synced-folder location.
- Drain drafts and request a bounded flush on lifecycle changes. Background execution
  is not guaranteed; recover from the last durable generation after termination.
- Release database handles and document ownership when suspending/closing as required
  by the lifecycle design. Test on physical devices before claiming safety.
- Recreate WebViews from the host's current subscribed view. Native ownership allows
  an engine without a renderer; it does not grant unlimited background networking.

## Later Tauri work

- Host the same Rust semantic core; bind the existing application SDK over Tauri IPC.
  Do not port a growing Swift interpreter into a second semantic implementation.
- Implement filesystem isolation, writer ownership, SQLite and lifecycle adapters for
  Windows/Linux. Unix `flock`, inode assumptions and AppKit APIs are not portable.
- Keep tokens outside authored web content. Validate operation envelopes at the host
  boundary, and give authored code no arbitrary persistence or native-command access.
- Re-run the bridge, Unicode, IME, process-death and performance suites on WebView2 and
  WebKitGTK. WKWebView timings cannot establish those platforms' behavior.

## Clean prelaunch cutover, only after conformance

Build the complete shared-core conformance suite before replacing production. Choose
one engine, update the SDK and templates together, rebuild fixtures, and remove the
superseded runtime path. No compatibility adapters or legacy document migration are
required by this spike. Preserve the current Mac product features during that cutover;
do not replace the application with the benchmark shell.

The implementation sequence should be:

1. Port the descriptor interpreter into Rust behind a generated binding. Use the
   current semantic fixtures as an independent oracle for reads, issues, identity,
   atomic writes and persistence. Establish counter behavior before sync work.
2. Complete the intent/subscription SDK and host session boundary. Prove stale-frame
   addressing, text drafts/IME, request retries, undo and renderer remount. Re-run
   performance with all those semantics enabled.
3. Integrate that single owner into the existing Mac UI, CLI and export paths.
   Preserve writer ownership and failed-save recovery; remove the superseded engine
   only once the Mac product checks pass.
4. Add durable synchronization against failure fixtures, then platform adapters for
   iOS and Tauri when those products are scheduled. Neither is a prerequisite for
   deciding the Mac owner now.

This avoids spending the next phase maintaining two production interpreters or
performing a large TypeScript refactor that would immediately be discarded.

## Failure isolation and opening costs

The native candidate moves CRDT decoding from WebContent into the application
process. That removes a renderer dependency but changes the crash boundary. Malformed
snapshots, allocation bounds, binding errors and panic behavior need native ownership
tests before a cutover; renderer-crash survival is not equivalent to engine-crash
survival. This prototype uses a serial document queue and Swift 5 language mode; a
production Swift 6 wrapper needs compiler-checked actor isolation and a complete
error/status publication path.

Current slop bundles already exclude the engine. Native ownership eliminates per-view
WASM loading/initialization, not an embedded WASM file inside each slop. It may reduce
working memory and some opening costs; host application download size is a separate
measurement. Engine distribution is already centralized today. Both designs need a
clear postlaunch contract for SDK and stored bytes, even though this prelaunch cutover
requires no legacy support.
