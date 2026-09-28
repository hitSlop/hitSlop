import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  selectedChannelId: "spacestation",
  favoriteChannelIds: ["spacestation", "missioncontrol", "deepspaceone", "dronezone"],
  volume: 0.72,
  muted: false,
} satisfies Input<typeof schema.fields.node>;
