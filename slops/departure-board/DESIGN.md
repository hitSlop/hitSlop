# Departure Board design

One job: see what is next today and flip a stop's status with one tap.

The object is a black airport board with amber heading, warm-white letters and
split-flap cells. Each character is its own flap: a top and bottom half that GSAP
rotates through a short run of letters to land on the new one. A new board clacks
into place row by row; an edited cell flips on its own. Status is a word ("ON TIME",
"BOARDING", "DELAYED", "DEPARTED") as well as a colour. The clock shows the current
time and is presentation only.

Saved: the title, and each stop's time, name and status. Flap progress, the clock,
sound and the edit sheet are never saved. Reduced motion skips the flips and shows
final text immediately. Sound is off until turned on; it is a short filtered tick
per flip and is closed when the window goes away. Export is a static board with no
animation, and the icon is three rows of flaps.
