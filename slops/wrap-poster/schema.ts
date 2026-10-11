import { defineDocument, s } from "hitslop";

export const shapeKinds = ["circle", "square"] as const;
export const tones = ["tomato", "cobalt", "mustard"] as const;
export const maxShapes = 5;
export const minSize = 0.12;
export const maxSize = 0.6;

const poster = defineDocument({
  title: s.text({ description: "The headline, flowed around the shapes." }),
  body: s.text({ description: "The body text, flowed around the shapes." }),
  // Centre and size as fractions of the page width (y: of its height), so a poster looks the same at any window size.
  shapes: s.list(s.object({
    kind: s.enum(shapeKinds),
    tone: s.enum(tones),
    x: s.number({ min: 0, max: 1, description: "Centre, as a fraction of the page width." }),
    y: s.number({ min: 0, max: 1, description: "Centre, as a fraction of the page height." }),
    size: s.number({ min: minSize, max: maxSize, description: "Diameter or side, as a fraction of the page width." }),
  })),
}, { initial: {
  title: "Why text wraps",
  body: `Every column of text is a negotiation. The page offers a width, the words ask for room, and somewhere in between a line breaks. For centuries that negotiation happened in lead and ink, one line at a time, and it still does: a line that cannot fit simply ends, and the next one begins where the last one stopped.

Here the page keeps changing its mind. Drag the circle and every line is measured again, not by asking the browser to reflow a paragraph, but by walking the words and spending only as much width as each row has left. Rows beside the shape get shorter; rows below it recover.

Move the square into a paragraph and watch the text split around it, left span first, then right, the way a magazine does it. Edit the words to make it yours.`,
  shapes: [
    { kind: "circle", tone: "tomato", x: 0.72, y: 0.4, size: 0.34 },
    { kind: "square", tone: "cobalt", x: 0.22, y: 0.76, size: 0.22 },
  ],
} });

export default poster;
