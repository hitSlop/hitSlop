# Hourglass

A sand glass on the desktop that drains toward one time: a focus block, a trip, a deadline.
The window is the object. Its silhouette (`glass.ts`) is two walnut caps, two thin posts and
the glass, and the gaps beside the posts are holes in the window. The glass is the host's
`glass` background: frosted desktop, tinted by the `glass` theme color, so lowering its
opacity in the theme panel clears the glass.

- **Readout:** a serif clock under a day (`59:59`), whole days beyond it, and the end time
  beneath. The title sits on the top cap.
- **Sand:** the top bulb holds what is left, at the level its volume fills in the curved
  glass. What has fallen heaps into a cone under the stream at its angle of repose, so a
  mound shows within the first minutes rather than a flat film. A thin stream falls while it
  runs.
- **Setting a time:** the whole glass becomes the picker. Durations sit in the top bulb and a
  date in the bottom one. The date is a native `datetime-local` input, because its calendar
  opens outside the window, where an in-page popover would be clipped by the silhouette.
- **Turn over:** restarts the same duration. Only the glass turns, 720 ms around its
  horizontal axis, while the stand stays still. Reduced motion skips the turn.
- **Saved:** the title, and the start and end as epoch milliseconds. The clock and the turn
  are local.
- **Reduce Transparency:** with it on, macOS draws the frost as a solid light surface under
  the tint, and the readout stays legible on it.
- **Captures:** they can't hold the frost, so the export and icon paint the glass. The export
  is a light card with the vessel beside the readout.
