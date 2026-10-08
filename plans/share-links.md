# Share links

Status: deferred proposal, re-staged 2026-10-06. It builds on the
[browser host](browser-host.md), which ships first as a beta, and waits for two things:

- **Launch.** It is not on the roadmap's Direction list, and AGENTS.md defers accounts
  and sharing.
- **A product decision on accounts.** [Ideas](../docs/ideas.md#what-we-wont-take) says
  hitSlop "stays local, with no account"; sender sign-in changes that.

A share publishes a snapshot behind `https://hitslop.com/s/<share-id>`. Recipients open
it through the browser host's import, or download it, or open it in hitSlop. Before
this, the same static host on hitslop.com can take a dropped `.slop` with no account; it
needs only hosting and the [hosted origin layout](#copy-origins-for-hosting).

## Agreed product choices

- A share publishes a fixed copy of the document's current contents. Recipients edit
  independent copies; neither their changes nor the sender's later changes propagate.
- The sender signs in to publish and manage links. Recipients need no account.
- Browser copies autosave locally and can be downloaded as `.slop` files. No cloud
  saving of recipients' edits, cross-device recovery, or live collaboration.
- Make Share visible in the floating document toolbar, immediately before `…`, and
  available through File → Share…. Keep the existing offline file-sharing action.
- An iOS app and App Clip are later clients. They are not prerequisites for this work.

## Sender and recipient flow

1. **Share** opens a popover; opening it uploads nothing. Explain: “Anyone with this
   link can make their own copy. Includes your document's current contents.” Offer
   **Create Link** and **Send File…**.
2. **Create Link** signs the sender in if needed, then returns to the document. Drain
   pending page edits and attachments, flush through the owner, and build the
   publication artifact ([Publication](#publication-artifact)). Never upload or copy a
   busy live SQLite file directly.
3. The sharing service accepts the authenticated upload, validates it without executing
   authored code, and stores an immutable artifact. An unguessable share ID resolves to
   it through an access-controlled share record:
   `https://hitslop.com/s/<share-id>`, with no local path or document contents.
4. Show **Copy Link**, **Send…** and **Stop Sharing**, plus the publication date. Send
   opens the native share picker with the URL. Failure offers retry, never a
   working-link state. Retrying one publication must not create duplicate shares.
5. Reopening Share retrieves the existing link. **Create New Link…** publishes a newer
   snapshot; it never changes what an old link means. The account keeps managing older
   links.
6. The share page shows the title, preview and **Use in Browser**, **Open in hitSlop**
   and **Download .slop**. It loads no WASM. Do not depend on installed-app detection;
   a user action invokes the app, with a browser/download fallback.
7. **Use in Browser** creates a new local copy from the artifact through the browser
   host's import. Returning to the share offers **Continue Your Copy** when one exists locally,
   plus starting another copy.
8. **Saved in this browser** appears only after a durable save. Opening a browser copy
   in the native app needs a downloaded file; the share URL alone cannot carry local
   edits.
9. Stop Sharing prevents new hosted access. Existing local and downloaded copies stay
   usable, and a browser “Your copies” page reaches local copies whether or not the
   share is still active.

## Copy origins for hosting

Hosting runs strangers' slops, so each copy's app needs its own origin, never the
host's.

- **L1:** copy origins as subdomains of the host's own site. The frame is same-site
  and not partitioned. The host keeps no cookie session; recipients have none.
- **L2:** copy origins on a separate registrable domain, the usual isolation. The frame
  is then third-party, where storage partitioning and Service Worker support differ by
  browser.
- **Fallback (b):** serve published apps' immutable assets over HTTP from the service,
  by app digest, with byte ranges from R2. The Service Worker is then needed only for
  copies opened from local files.
- The copy frame keeps `allow-same-origin`, which is safe because its origin is never
  the host's.
- Choose the host origin once. OPFS data belongs to an origin, so moving it strands
  every browser copy.

Before share links ship, rerun the
[storage spike](../archive/docs/evidence/browser-storage-2026-10-06.md)'s deferred cases against a
deployed harness on two domains: L1 and L2, mobile Safari and Android Chrome, private
windows, `persist()`/`persisted()` per browser, and background-tab autosave. Add the
**Use here** tab handoff.

## Publication artifact

The owner already makes the artifact. `Request::Copy` (`crates/hitslop-core/src/owner.rs`)
waits for the save, then `Store::copy_clean` writes a new file in one transaction:

- the current state without history (a shallow snapshot at the latest frontiers), so
  deleted content does not travel;
- only the attachments the state references (`Document::attachment_references`, the scan
  close-time reclamation uses: an ID anywhere in a string, markdown included);
- the artwork it is given, in place of the original's.

Native Duplicate and Send File use it, and the browser host's `slop open --browser` reaches it
through a `copy` socket request. Publication adds only:

- **Artwork** rendered for the published state, passed to the copy.
- **Checks.** The artifact passes every open check, or publishing is refused with a
  clear reason. Invalid stored state is never repaired, and `$id`s are preserved.
- **The upload and the share record.** Share ownership lives outside authored state and
  in the account, so links stay manageable after the local file moves or is deleted.

## Sharing service and native integration

- **Cloudflare HTTP service with R2 artifacts**, matching the roadmap.
  - The Worker handles authenticated publication create and finalize, the sender's
    share list, anonymous capability reads and downloads, and owner-only revocation.
  - Uploads go to R2 through presigned URLs.
  - TypeBox owns every request and reply. Retries are idempotent.
- **Validation runs `slop-engine inspect`** in a Linux container; the engines workflow
  already builds `linux-x64` and `linux-arm64`. These are the same native checks the
  CLI runs, on real SQLite, executing no authored code. A WASM validator inside a
  Worker would hit the Worker memory limit at the largest documents. A share becomes
  readable only after validation passes.
- **Access control.** Artifacts stay private behind the share record. Public object
  URLs and caches must not bypass revocation. An unlisted link is bearer access, not
  an identity-restricted invitation. No public catalog.
- **Native Share flow.**
  - The toolbar button and File → Share… reach the same popover.
  - Sign-in and upload recover from failure.
  - Copy and Send the link; manage older links.
  - Keep offline Send File.
  - **Open in hitSlop** from the web passes the share ID to the app, which downloads
    and opens a new document.
- **Capabilities.** The first [slop capabilities](slop-capabilities.md) release adds
  camera/microphone declarations without per-slop consent. A separate per-code consent
  and revocation design must land before hosted links distribute capture-capable slops.
  Camera/microphone access needs explicit policy delegation to isolated browser app
  frames and browser permission tests; future native-only capabilities need explicit
  unavailable behavior in the browser.
- **Decisions to settle first:** account provider, metadata store, container host,
  upload limits and retention.

## Milestones and acceptance

1. **Share links.** The publication artifact, the service, validation, revocation, the
   native Share flow, and the share page feeding the browser host's import.
2. **Open in hitSlop** from the web, and older-link management polish.

Before shipping, exercise these cases:

- Interrupted uploads and downloads, and revoked links.
- Artifacts containing no deleted text and no unreferenced attachments.
- One recipient's edits never reach another recipient or the publisher.

Share links bring accounts and hosted sharing forward. Update AGENTS.md, architecture, the
engineering contract and the roadmap together when it lands.

## Deferred

Live rooms, sync, cloud recipient copies, presence, shared undo, public discovery and
App Clips stay separate. An App Clip can later hand a native `.slop` to the full iOS
app through an App Group container; its local data stays purgeable.
