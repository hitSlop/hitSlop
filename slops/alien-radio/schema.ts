import { defineDocument, s } from "hitslop";

export default defineDocument({
  selectedChannelId: s.string({ description: "The tuned SomaFM station id, such as spacestation." }),
  favoriteChannelIds: s.list(s.string(), { description: "SomaFM station ids the transport arrows cycle through." }),
  volume: s.number({ min: 0, max: 1, description: "Gain from 0 to 1." }),
  muted: s.boolean({ default: false }),
}, { initial: {
  selectedChannelId: "spacestation",
  favoriteChannelIds: ["spacestation", "missioncontrol", "deepspaceone", "dronezone"],
  volume: 0.72,
} });
