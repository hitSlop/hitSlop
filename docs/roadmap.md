# After the local release

The current launch includes the signed Mac app and matching npm authoring packages. Users create local documents from bundled or installed templates, edit through the UI or native CLI, and export PNG/PDF. Additional authored templates can join the collection independently of hosted services.

## Hosted templates

A future publishing service may accept validated immutable `hitslop-v1` artifacts, expose a generated OpenAPI catalog, and download immutable cached masters. Creating a document will still make a writable local copy; bundled and cached masters must work offline.

Plan a separate Cloudflare HTTP module with R2 artifacts and catalog metadata. Keep TypeBox authoritative. Publisher identity, upload limits, package isolation/checksums, immutable release identity, and abuse controls are prerequisites. Any historical worker retained in a local `deferred/` archive is unsupported scaffolding, not a v1 deployment or self-hosting path. The local app needs no document server.

## Collaboration

Collaboration is separate from hosted discovery. A future design can authenticate in Swift, transfer Loro updates between replicas, and persist opaque updates remotely. Each local replica retains one writer and local SQLite storage. History pruning remains deferred and will need a retention policy compatible with offline replicas. Mergeable map children, splice-based text binding and commit origins are already in place. Undo first needs `change()` staging that a Loro `UndoManager` can observe; the current fork-based staging hides local operations. Keep credentials outside authored code, preserve ByteStore as the persistence boundary, and introduce a dedicated sync envelope rather than overloading `apply`.

Do not restore JSON room seeds, command/snapshot authority, guest snapshot reconciliation, JavaScriptCore, data.json, or a second semantic validator. Convergence tests around internal import/export do not constitute a shipped collaboration product.

Media import, account UI/Auth/App Check, undo UI, schema evolution, history pruning, iCloud and other synced folders, and other native platforms remain deferred. Historical source may be retained in the optional, Git-ignored `deferred/` archive; it is not available in fresh clones. There is no automatic migration from pre-v1 documents.
