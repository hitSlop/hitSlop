# Wrap Poster design

One job: make a one-page poster where the words and the shapes share the page.

The object is a paper poster (520 × 700 logical pixels, scaled to the window) on a dark
desk, with a serif headline and body text in Lora and flat tomato, cobalt and mustard
shapes. Pretext measures the words once (when the text or font changes) and then lays
them out row by row on every drag: each row is cut by the shapes, and every free span
wide enough for a line gets one, left to right, so text splits around a shape in the
middle of a paragraph. The headline uses the same engine as the body.

Saved: title, body, and each shape's kind, colour, centre and size as fractions of the
page. The editor page is a fixed size so resized windows keep the same layout.
Dragging updates a live position; it is saved when the pointer is released. Arrow keys
move a selected shape, plus and minus resize it, Delete removes it. If the words no
longer fit, the bar says so in words. Selection, the live drag and the open text sheet
are view state; the edits live in `poster.ts`, shared by the page and `commands.ts`. Export uses a synchronous paper composition: the saved shapes lead the page,
followed by the complete title and body in Lora. It keeps every word without the
browser-only wrapping measurement. The icon is a page with a circle.
