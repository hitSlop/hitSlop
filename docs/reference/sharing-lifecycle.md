# Sharing receipts, outgrowth and recovery

How a shared document stays safe on the device: what "synced" means, what happens when
a document outgrows sharing, and where unsynced edits go when access ends. Identity,
routes and limits are in [cloud documents](../cloud-documents.md).

## The confirmed receipt

Each device keeps a small receipt per account and cloud document in its library’s
`index.sqlite`, naming the one document version that **both** the cloud holds and the local save
holds. The
owner writes it only when the cloud has acknowledged every push, the session is caught
up and not paused, and the saved file's exact history matches. A seed upload also writes
it once Open succeeds, which follows the upload of every attachment the seed references; a
download writes it once its attachments are verified and persisted. The receipt is replaced in an index transaction; a failed
write rolls back and keeps the previous one. It holds no document state and no credentials.

The receipt is the only thing that decides whether a downloaded copy is clean. **Remove
Download** is refused while the saved version differs from the receipt, and catalog
status compares the saved version with it. A missing receipt means unconfirmed.

## Initial upload

The creator stages one immutable local snapshot of the saved document. The transmitted
seed is SQLite with the complete embedded app and retained history; attachments are not
part of it, and neither is artwork. In order, the creator uploads the seed
and every attachment the seed references, all from that snapshot, so later
edits, imports or removals in the live document do not change what is sent. The seed and
attachments use the same content-addressed blob upload route. Only then does it call Open,
naming the seed's SHA-256, version, app digest and exact attachment hashes. Open verifies
every named blob's presence, recorded size and available R2 checksum, enforces the attachment
budget and commits the set atomically. Unused staged blobs enter durable garbage collection.
The seed is excluded from the attachment budget and cannot change once live. The owner binds
to the document only after Open.

| Failure | Result |
|---|---|
| A seed or attachment upload fails | The document stays uploading and the owner unbound. A retry stages a fresh snapshot and sends only missing blobs. Staged blobs consume no attachment budget; the eventual Open selects the seed and attachments and discards the rest. |
| The seed exceeds 16 MiB | Refused before the cloud document is created; the local document is retained. |
| The document grew past 16 MiB after going live | Enabling sync again stages nothing and reconnects; the base limit under [Outgrown](#outgrown) still applies. |

The 16 MiB limit covers the embedded app, history and SQLite overhead; attachments have
their own budgets. A document beyond it cannot start sharing (a room that is already live keeps reconnecting), so keep large media in
attachments. A valid local app can exceed the limit within its 50 MiB authoring budget,
and `pop build` warns once it stores 12 MiB of resources. Initial downloads fetch and
verify attachments before they become ready, including when the room has no subsequent
edits. An interrupted transfer remains retryable rather than publishing an incomplete
download.

## Outgrown

Compaction only folds. When a document's base would exceed 16 MiB, or one change cannot
be relayed, sync pauses as `Outgrown`: nothing is lost, local edits keep saving to the
file, and the person can choose **Save Unshared Copy…** to continue in a new document.
There is no history replacement. A full attachment budget pauses as `FilesFull`.

The other pauses are `RoomFull`, `NeedsUpdate` (a newer marker; update the app),
`Refused`, `MissingHistory` and `Ended` (sharing stopped). A pause keeps its reason until
the next connection.

## Recovery

Edits are never discarded by a cloud decision. When the cloud document is deleted or
the account's access is definitively denied, the catalog entry is removed after the
open owner is flushed, and a downloaded file that still holds edits becomes a local
document titled **Recovered edits**. Recovery copies are account-scoped: they appear on
sign-in, hide on sign-out, are indexed again after a restart and are never uploaded
automatically. A guest whose invitation is revoked keeps the downloaded file and can save an
independent copy.

## What survives which failure

| Failure | Result |
|---|---|
| Offline or dropped socket | Edits save locally; the next connection's hello exchange sends what the cloud lacks. |
| Crash before the cloud acknowledged | The file has the edits; the receipt still names the older version, so the copy is unconfirmed and syncs on next open. |
| Receipt write fails | The previous receipt stays; the version is simply unconfirmed. |
| Failed local save | The owner keeps ownership and shows a native retry. |
| Outgrown or full attachments | Sync pauses; local edits keep saving; fork with Save Unshared Copy…. |
| Revocation, leaving or deletion | Local file kept; a deleted cloud document leaves a Recovered edits copy. |
| Catalog write fails | The pending write is retried; editing and the connection are unaffected. |
