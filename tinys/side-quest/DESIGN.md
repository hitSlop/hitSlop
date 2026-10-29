# Side Quest design

A graph-paper notebook page with a spiral binding and red margin rule. Each
course gets a rotated washi-tape label and a gel-pen trail through its quests,
drawn solid behind today and dotted ahead. A highlighter band and the player
sprite mark today; the highlighted "Next boss" readout is the one number that
matters. Bosses are wobbly SVG monsters with an HP bar. A K.O. stamp and
greyscale mark a defeated boss. Cleared quests turn into emoji stickers with a
white die-cut edge.

The sticky-note panel holds editing: quest details and the "Log study session"
hit button, or side quests and the sticker tray when nothing is selected. It is
hidden on export. The export is the whole map plus a side-quest list, meant to
be shared as a semester recap. Narrow windows stack the panel under the map, and
the map scrolls horizontally instead of crushing its timeline.

Boss HP is `maxHp - hits`, and `hits` is a counter so concurrent study sessions
from a future study-group "raid" merge without loss.
