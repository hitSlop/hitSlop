# Little Days

One baby, one local journal: record feeding, diaper changes, manually timed sleep,
and memories with the caregiver who was there. No accounts or synchronization.

The standard resizable window opens at 900 × 640. Warm cream paper, Fredoka
headings, Outfit body text, and small line illustrations echo a baby keepsake
book. Peach, pink, blue, lilac, mint, yellow, and apricot personalize the profile;
care types keep stable pastel colors and labeled icons. Fonts are bundled with
OFL licenses. Motion is limited to brief control feedback with reduced-motion
support.

At the default size the profile, recent care, memory, caregivers, entry actions,
date controls, and note action remain in view. Only the timeline scrolls as the
history grows. Narrow windows stack content and scroll naturally. Long notes
wrap, while the memory preview is limited to two lines and opens the full entry.

Each document starts blank with one editable caregiver named You. Caregivers are
archived, never deleted; entries refer to their persistent row IDs. Profile and
caregiver names edit directly through text bindings. Entry dialogs commit only
when Add/Save is pressed; Cancel discards unsubmitted form values. Accepted
changes use typed, atomic document operations. No routine persistence status is
rendered in the app.

Sleep appears on its local start date, with its full duration included in that
page’s summary. Overnight end dates are explicit. Instants display in the device’s
current time zone. New entries start at now; after adding/backdating, the journal
opens the entry’s day. The latest feeding/diaper and memory ignore that date filter.
Bottles retain their original mL or US fl oz values; no inferred conversion.

Photos are validated JPEG/PNG/WebP attachments, never document base64. Profile
photo removal clears the reference; attachment cleanup remains host-owned.

PNG/PDF exports show the selected day’s full journal in normal flow. Closed
exports default to today. The book-and-sun icon is independent of editor state.
