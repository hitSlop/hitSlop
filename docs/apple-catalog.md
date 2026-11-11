# Apple catalog

The iPhone catalog uses large artwork tiles, centered names, cream surfaces and pink
controls. The Mac keeps its sidebar, document list and permanent preview. Both use the
same cached document presentation model. Editor, owner, storage and save behavior stay
with their existing boundaries.

## Inbox and Recents

Inbox contains cloud documents from other devices that have not been opened on this
device. It appears above Recents with a count badge. Recents contains documents used
here. Local creation and adoption do not add an Inbox item.

Rust stores the opened flag in the device's library index. Receiving a new remote
reference creates an unopened entry. Registering its real document owner marks it
opened; downloading alone does not. Metadata refresh and removing a download never
reset this flag. Inbox state belongs to this device, not the cloud library or the shared
document.

## Document badges

One small circular badge sits at the artwork's lower-right corner:

| Priority | State | SF Symbol |
| --- | --- | --- |
| 1 | Needs attention or unavailable | `exclamationmark.triangle.fill` |
| 2 | Not downloaded | `arrow.down.circle` |
| 3 | Shared by the owner or joined as a guest | `person.2.fill` |
| 4 | Ordinary local or private sync | None |

The symbols are neutral cloud, download and attention glyphs. Routine device badges, sync
checkmarks and storage subtitles are omitted. Pending
sync alone is not an exception badge. Detailed sync status, help and retry remain in
the existing status sheet. A shared badge describes the saved relationship; it makes
no claim about connectivity, presence or active editors. VoiceOver retains both the
sharing relationship and exception description when an exception takes visual priority.

The account-scoped Rust library index stores nullable `sharing_enabled`. An unknown
owner state does not imply sharing. Successful existing authority access responses
refresh this hint during sync, download, join, access checks and sharing changes.
Transient failures retain the last known value. An optional presentation refresh must
not make an acknowledged upload fail. No authority request runs while rendering a cell
or refreshing the local index.

Rust projects sharing from the accepted document UUID, the current account and cached
library relationship; filenames do not establish sharing. The long-lived Rust library
fences account changes and reattaches registered live owners. Opening an arbitrary Mac
file records it in Recents without adopting it into the library.

## Sync and accounts

Cloud transport is event driven, with no polling of the service: one Rust account connection
delivers the person's library metadata. Entries carry each document's sync phase, pause reason and detail; one library callback invalidates their snapshots. Native
refreshes follow that callback, document save notifications and one managed-folder watcher
using vnode notifications, including writes made by another process. Bursts are coalesced.
Library work has its own queue, separate from document opening. The Mac and iPhone use one default
Firebase app (Authentication, Analytics, Crashlytics); telemetry collection is
on only in Release builds. Cloud documents are described in [cloud documents](cloud-documents.md).

## Layout and interaction

iPhone Recents and Templates use lazy icon grids: three columns at ordinary portrait
text sizes, fewer columns on narrow layouts or at large accessibility sizes. Recents
artwork is about 90 points, softly rounded and shadowed. Inbox is a horizontal shelf
with smaller, roughly 70-point artwork. Names are centered with compact two-line
leading and more space at accessibility sizes. Header and row gaps stay compact.
Search is tall and rounded; Settings
and Refresh use the pink accent. The native floating Recents/Templates tab bar remains,
with quiet sync status placed above it through safe-area layout.

Template categories, preview and creation screens, opening, download, refresh, search
and haptics remain. Long-press menus and equivalent accessibility actions replace swipe
actions. First-use and no-search-results states are distinct.

On iPhone a tapped tile zooms into its document at once, showing the tile's artwork until
the page is ready and padded for the safe area. Swiping back from the edge, swiping down
on the page or Back to Library returns to the catalog immediately: the page saves while it
animates out, and widgets, artwork and release finish in the background behind a quiet
Saving mark on the tile. A close that cannot save keeps the document's owner and every
edit, and marks its tile Not saved with Retry Save; tapping the tile brings the page back.

Mac list artwork is modestly larger, with softer tiles and selection. Routine sync and
storage subtitles are removed. Duplicate document names retain folder disambiguation;
full paths stay available in tooltips and details. Keyboard selection, shortcuts,
previews, context menus and creation/opening behavior remain.

## Qualification

Tests belong to Rust library persistence and the shared native presentation boundary.
They cover private, shared and unknown state; state changes; cache reopening; account
isolation; Inbox persistence; badge priority; and accessibility text.
No per-template tests are added.

Visual qualification covers local, private-sync, shared, undownloaded, unavailable and
failed-sync documents, plus long and duplicate names, search, empty states, large text,
dark mode, keyboard navigation and VoiceOver. Run affected checks, `bun run verify`
and finally `bun run verify --native`; build and inspect the iPhone app.
