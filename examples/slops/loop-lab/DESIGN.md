# Loop Lab design

One job: stack a few loops and hear how they fit.

The object is a dark violet lab with one card per track: drums in coral, synths in blue, each
with a pattern field in monospace, a name, a voice (for synths), mute and remove, and three
sliders (volume, filter, reverb). The pattern field is Strudel mini-notation, committed
when you leave the field or press Return; a pattern Strudel can't read is skipped and the
track says so in words. A thin bar shows the position in the current cycle. A one-line cheat
sheet sits under the tracks.

Saved: tempo and, per track, its kind, name, notation, voice, volume, filter, reverb and mute.
Play state, the cycle bar and sample loading are never saved. Strudel is imported on the
first Play (a user gesture), so nothing sounds or reaches the network before then; it stops
when the window is hidden or closed. Edits apply on the next cycle while it plays. Export
lists the tracks and their notation; the icon is three pattern rows.
