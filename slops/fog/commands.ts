import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import schema from "./schema";
import { clearSnapshot, mirrorView, setBrush } from "./mirror";

// Commands edit document verbs only. The mirror needs a camera, so fogging, drawing and
// saving a snapshot happen in the page; an agent can attach an image with `slop batch --attach`.
export default defineCommands(schema, {
  setBrush: {
    description: "Set the pinch-drawing brush radius in CSS pixels (7 to 38).",
    args: { radius: s.integer() },
    run: ({ tx }, { radius }) => setBrush(tx, radius),
  },
  clearSnapshot: {
    description: "Forget the last saved snapshot; its image is dropped from the file when the document closes.",
    args: {},
    run: ({ tx, current }) => clearSnapshot(tx, current),
  },
  snapshot: {
    description: "Return the last saved snapshot's name, size and time, or null when there is none.",
    args: {},
    run: ({ current }) => {
      const { snapshot } = mirrorView(current);
      return snapshot ? { name: snapshot.name, width: snapshot.width, height: snapshot.height, byteLength: snapshot.byteLength, at: snapshot.at } : null;
    },
  },
});
