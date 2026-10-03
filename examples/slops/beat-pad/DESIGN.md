# Beat Pad design

One job: make a loop in a minute and hear it.

The object is a small studio pad on a dark violet field. Sixteen steps are shown as two
bars of eight so each cell is a full 44px target; four tracks have their own colour
(kick coral, snare yellow, hat teal, bass violet), and a lit step is also marked with a
dot, or with its note number on the bass, so state never rests on colour alone. The
playhead is an outline that walks the grid. Tempo and swing are sliders that sound
immediately and save on release; the kit switches the whole voice set.

Saved: tempo, swing, kit and the four step strings. The playhead, the Play state and the
audio engine are never saved. Tone.js is imported on the first Play (a user gesture), runs
in its own audio context and is closed when the window closes or is hidden. Export is the
grid with lit steps and the tempo line; the icon is a four-row grid.
