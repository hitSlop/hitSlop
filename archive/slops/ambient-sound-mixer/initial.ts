import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  preset: "Lo-Fi Rain",
  master: 80,
  playing: false,
  channels: { rain: 75, thunder: 20, wind: 35, birds: 0, night: 40 },
  muted: { rain: false, thunder: false, wind: false, birds: false, night: false },
  soloed: { rain: false, thunder: false, wind: false, birds: false, night: false },
} satisfies Input<typeof schema.fields.node>;
