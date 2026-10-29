import { defineDocument, s } from "@hitslop/tinys";
import { stationIds } from "./station-list";

export const bands = ["preamp", "hz60", "hz170", "hz310", "hz600", "hz1000", "hz3000", "hz6000", "hz12000", "hz14000", "hz16000"] as const;

const band = () => s.number({ min: 0, max: 100, default: 50 });

export default defineDocument({
  selectedStationId: s.enum(stationIds, { default: "groovesalad" }),
  volume: s.number({ min: 0, max: 100, default: 45 }),
  balance: s.number({ min: -100, max: 100, default: 0 }),
  milkdropOpen: s.boolean({ default: false, description: "Whether VIS was left open; Reduce Motion keeps it closed." }),
  equalizer: s.object({
    on: s.boolean({ default: true }),
    preamp: band(), hz60: band(), hz170: band(), hz310: band(),
    hz600: band(), hz1000: band(), hz3000: band(), hz6000: band(),
    hz12000: band(), hz14000: band(), hz16000: band(),
  }),
  skin: s.object({
    id: s.optional(s.string({ description: "Attachment ID of the imported classic .wsz skin." })),
    name: s.optional(s.string()),
  }),
});
