# Sharing receipts, outgrowth and recovery

How a shared document stays safe on the device: what "synced" means, what happens when
a document outgrows sharing, and where unsynced edits go when access ends. Identity,
routes and limits are in [cloud documents](../cloud-documents.md).

## The confirmed receipt

Each device keeps a small receipt per relay endpoint and cloud document, in its local state
directory, naming the one document version that **both** the cloud holds and the local save
holds. The
owner writes it only when the cloud has acknowledged every push, the session is caught
up and not paused, and the saved file's exact history matches. A seed upload or
download also writes it. The receipt is replaced atomically under an OS lock; a failed
write keeps the previous one. It holds no document state and no credentials.

The receipt is the only thing that decides whether a downloaded copy is clean. **Remove
Download** is refused while the saved version differs from the receipt, and catalog
status compares the saved version with it. A missing receipt means unconfirmed.

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
