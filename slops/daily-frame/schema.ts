import { defineDocument, s } from "hitslop";

/** One document file holds at most 256 attachments (host limit), so at most 256 frames. */
export const maxFrames = 256;
/** One attachment holds at most 10 MiB (host limit). Frames are JPEGs of about 0.1 to 0.4 MB. */
export const maxFrameBytes = 10 * 1024 * 1024;

export default defineDocument({
  title: s.text({ description: "What the frames follow, shown as the heading." }),
  // One frame per local day, in date order. The photo bytes live in the document's
  // attachments; a frame keeps only the reference.
  frames: s.list(s.object({
    date: s.string({ minLength: 10, maxLength: 10, description: "The local day the photo belongs to, YYYY-MM-DD. One frame per day." }),
    attachment: s.object({
      id: s.string({ description: "The attachment ID of the JPEG photo." }),
      mimeType: s.string({ description: "The photo's media type, as the host verified it." }),
      byteLength: s.integer({ min: 0, description: "The photo's size in bytes." }),
    }),
    note: s.text({ description: "A short caption written on the print." }),
  })),
}, { initial: {
  title: "Basil on the kitchen sill",
  frames: [],
} });
