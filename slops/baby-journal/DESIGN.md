# Little Days

One baby, one local journal: record feeding, diaper changes, manually timed sleep,
and memories with the caregiver who was there. No accounts or synchronization.

The standard resizable window opens at 900 × 640. Warm cream paper, Fredoka
headings, Outfit body text, and small line illustrations echo a baby keepsake
book. Peach, pink, blue, lilac, mint, yellow, and apricot personalize the profile;
care types keep stable pastel colors and labeled icons. Fonts are bundled with
OFL licenses. Brief surface and control motion respects reduced motion. Native
haptics accompany changed choices, accepted entry/caregiver submissions and their
validation failures; feedback never runs from a document observer.

At the default size the profile, recent care, memory, caregivers, entry actions,
date controls, and note action remain in view. Only the timeline scrolls as the
history grows. Compact editors put a two-column set of care actions above the daily timeline.
Care & profile opens a secondary sheet with the profile, recent care, memory, and
caregivers. One modal contains the overview, entry, profile, and caregiver screens;
Back/Done returns to the previous screen and retains the entry draft while managing
caregivers. Forms become bounded sheets with scrollable content and reachable actions.
Opening a compact sheet focuses the sheet without raising the keyboard. Sheets and
menus animate in and out, and phone overlays use a single dimming layer. Long notes
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

PNG/PDF exports show the whole journal in normal flow, grouped by day, newest
first, with each day's feeds, changes, and sleep. The export cannot see the
editor's day choice, so it always shows every day. The book-and-sun icon is
independent of editor state.
