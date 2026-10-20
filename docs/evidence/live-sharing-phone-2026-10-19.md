# Account sharing on a physical Mac and iPhone — 2026-10-19

The signed development Mac app and a wired iPhone 13 mini running iOS 26.6.2
used the same isolated diagnostic library namespace. Normal library cursors were
not removed or reset. iPhone Mirroring was explicitly authorized; notification
forwarding was declined. No account IDs, invitations or tokens are retained here.

## Confirmed on the devices

- Sign in with Apple through the grouped development app IDs produced the same
  Firebase account on both platforms. Each app independently verified its refreshed
  Firebase session through the deployed Rust Worker.
- A new synced checklist created on the Mac was discovered on the phone through
  CloudKit. Opening it recovered creator access from the Account DO without copying
  an invitation to the phone. The phone exposed Copy Link and Stop as the owner.
- Mac and phone edits converged in the open editors. Opt-in Rust transport traces
  distinguish relay records from whole-file CloudKit delivery. One phone send lost
  its socket, reconnected and resent successfully; later records reached the editor.
- The phone backgrounded normally, the Mac added an eighth row, and foregrounding
  the existing phone process caught up to eight rows without another link or sign-in.
- Stop from the phone revoked the room on both devices. Both retained their editable
  local document. The Mac then accepted a ninth, local post-Stop row.
- Updating and relaunching the phone app retained its saved file and account session.
- With the startup fix, a direct cold Mac file-open accepted the previously refused
  checkpoint, retained its local ninth row and uploaded it automatically. The phone
  accepted that checkpoint and showed nine rows. Both persisted their cursors again;
  neither normal nor diagnostic cursors were deleted.
- Re-share from the Mac created a replacement room for the same document. Opening
  sharing controls on the phone recovered the new invitation from its ended predecessor
  without a link. Both reported Synced and a tenth row arrived through the replacement.
- After a normal phone background flush, the process was deliberately terminated and
  relaunched with its owner still open at termination. Opening the same library item
  retained the phone's checkbox edit and caught up to an eleventh Mac row. Transport
  traces confirm the refreshed connection received relay records and reached CaughtUp.
- Mac File → Join Shared Slop accepted the copied invitation and reused the already-open
  file/owner at the same UUID/path, preserving all eleven rows. This verifies native
  submission/reuse, not a signed-out guest or second-account device.

## Timing observations

Payload-free `HITSLOP_SHARING_TRACE=1` logs contain fixed event names and wall-clock
milliseconds. The flag is off by default. Same-process send-to-ack samples were:

| Device | Sample | Elapsed |
| --- | --- | ---: |
| Mac | Added row | 83 ms |
| Phone | Retried checkbox push after reconnect | 138 ms |
| Mac | Return checkbox edit | 152 ms |
| Mac | Edit while phone was backgrounded | 138 ms |

These few samples include the upload-before-push step. They are acknowledgement
round trips, not one-way network, visible edit or paint latency. Device clocks were
not calibrated. The phone's first traced bootstrap took 2.135 s and initial inventory
2.149 s; a later resume bootstrap took 683 ms. Cold opening is a separate experience
from steady-state editing. No percentile or service-level claim follows from this run.

## CloudKit startup failure and regression

Opening the Mac file directly during cold launch could precede the asynchronous
iCloud library connection. `SlopLibrary.register` then found no active library and
discarded the registration. Incoming checkpoints subsequently reported
`document has a live writer`, while the relay continued working through that writer.
The library repeatedly retried the refused checkpoint and withheld its durable cursor.

The native registry now remembers weak owner references and registers matching
writers before starting the library transport, including reconnects. Rust still owns
the writer and all merges. There is no alternate writer, lock removal or document
mirror. The Swift integration regression receives a real older checkpoint with a
locally edited owner already open: it fails on the previous implementation with the
observed busy error, then passes while preserving the local edit and writer lease.

The subsequent physical repeat passed post-Stop delivery and re-share as recorded
above. It reused the existing diagnostic namespace and saved engine state.

## Phone presentation

The editor is now a real navigation destination over the retained catalog. iOS owns
the push/back transition and default Back control; programmatic navigation respects
Reduce Motion. A failed close keeps the writer and returns to the editor with the
existing retry/error presentation.

The page covers the display using `viewport-fit=cover`, the native view's container
safe-area override and no automatic WebKit scroll insets. The toolbar has no opaque
background or repeated document-title header; iOS 26 supplies the floating Liquid Glass
Back and actions controls. The first on-device pass still showed white bands. The
viewport/inset correction removed them, and the user accepted the resulting layout.

Opening shows a blocking `Opening slop…` view until the existing session's page-ready
callback; document actions are disabled during preparation. A valid saved copy stays
editable during subsequent CloudKit/relay catch-up. Ordinary sync does not turn a
local-first document into an online-only editor. Physical edge-swipe validation is
explicitly left to the user; it was not established through Mirroring.

## Remaining qualification

- Signed-out guest invitation on each device and deliberate offline edit/reconnect.
- Physical edge-swipe gesture, left to the user. Native Back and opening/closing were
  exercised on the phone.
- The account-backed deployed browser campaign, full-history rescue, account deletion
  and Apple revocation, remaining abuse controls, privacy updates and frozen network
  fixtures remain public-beta gates. Earlier preview results are separate evidence.

The CI type failure in the Miniflare R2 inspection depended on an undeclared
`@cloudflare/workers-types/experimental` package. The pinned Workers types are now an
explicit development dependency. Local type verification passes; CI confirmation is
still required.
