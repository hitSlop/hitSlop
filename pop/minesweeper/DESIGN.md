# Buoy Sweep design

One job: a quick, finishable game of minesweeper that remembers where you stopped.

The object is a harbor chart: cream chart paper on a deep-sea field, pale-water
cells you clear, and orange buoys where you suspect a mine. Three sizes (6×6,
8×8, 10×10); the 10×10 board gives up the 44px target for fit, the others keep it.
The first reveal and its neighbors are always safe. Tap clears; the Buoy toggle,
a long press or a secondary click places a flag. Counts are digits, flags carry a
glyph and the status line says won or lost in words, never color alone.

Saved: level, status, board, mines, the clock and best times per size, wins and
losses. The mine layout is saved so a reopened game continues, not re-rolls. The
ticking display is presentation; the clock is saved with each move. Reduced motion
removes the pop-in. Export shows the board and the record; the icon is a small grid
with one buoy.
