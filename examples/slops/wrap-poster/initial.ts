import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  title: "Why text wraps",
  body: `Every column of text is a negotiation. The page offers a width, the words ask for room, and somewhere in between a line breaks. For centuries that negotiation happened in lead and ink, one line at a time, and it still does: a line that cannot fit simply ends, and the next one begins where the last one stopped.

Here the page keeps changing its mind. Drag the circle and every line is measured again, not by asking the browser to reflow a paragraph, but by walking the words and spending only as much width as each row has left. Rows beside the shape get shorter; rows below it recover.

Move the square into a paragraph and watch the text split around it, left span first, then right, the way a magazine does it. Edit the words to make it yours.`,
  shapes: [
    { kind: "circle", tone: "tomato", x: 0.72, y: 0.4, size: 0.34 },
    { kind: "square", tone: "cobalt", x: 0.22, y: 0.76, size: 0.22 },
  ],
} satisfies Input<typeof schema.descriptor>;
