import { defineDocument, s } from "hitslop";

export const maxTiles = 24;
export const maxCaption = 60;

export default defineDocument({
  title: s.text({ description: "The board's title." }),
  // Each tile points at an image the host stores; the bytes are never in the document.
  tiles: s.list(s.object({
    image: s.object({
      id: s.string({ description: "The attachment ID of the image." }),
      mimeType: s.string({ description: "The image's media type, as the host verified it." }),
    }),
    caption: s.string({ maxLength: maxCaption }),
  })),
}, { initial: {
  title: "Moodboard",
  tiles: [],
} });
