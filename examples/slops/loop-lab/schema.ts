import { defineDocument, s, type Value } from "@hitslop/document";

export const kinds = ["drums", "synth"] as const;
export const voices = ["sawtooth", "square", "triangle", "sine"] as const;
export const maxTracks = 6;

const schema = defineDocument({
  bpm: s.integer({ min: 60, max: 180 }),
  tracks: s.list(s.object({
    kind: s.enum(kinds),
    name: s.string({ maxLength: 20 }),
    // Strudel mini-notation, e.g. "bd*2 ~ sd" for drums or "c3 <e3 g3>" for notes.
    code: s.string({ maxLength: 160 }),
    voice: s.enum(voices),
    gain: s.number({ min: 0, max: 1 }),
    filter: s.integer({ min: 200, max: 12000 }),
    room: s.number({ min: 0, max: 1 }),
    muted: s.boolean(),
  })),
});

export type Loops = Value<typeof schema.descriptor>;
export type Track = Loops["tracks"][number];
export default schema;
