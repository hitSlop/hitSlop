import { defineDocument, s } from "hitslop";

/** Brush radius in CSS pixels, as the brush slider offers it. */
export const BRUSH = { min: 7, max: 38, default: 15 } as const;

// Only the brush and the last saved snapshot are kept. The fog, the camera and
// microphone streams and the hand landmarks are live view state and never saved.
const fog = defineDocument({
  brush: s.integer({ min: BRUSH.min, max: BRUSH.max, default: BRUSH.default, description: "Brush radius for pinch drawing, in CSS pixels (7 to 38)." }),
  snapshot: s.optional(s.object({
    id: s.string({ description: "The attachment ID of the saved PNG." }),
    name: s.string(),
    mimeType: s.string(),
    byteLength: s.integer({ min: 0 }),
    width: s.integer({ min: 1, description: "Image width in pixels." }),
    height: s.integer({ min: 1, description: "Image height in pixels." }),
    at: s.string({ description: "When it was saved, as an ISO 8601 timestamp." }),
  }), { description: "The last saved frame of the fogged mirror (an attachment reference). Saving again replaces it." }),
}, { initial: { brush: BRUSH.default } });
export default fog;
