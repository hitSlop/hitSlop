import type { Input } from "@hitslop/document";
import schema from "./schema";

const fresh = (text: string) => ({ text, revealed: false, done: false });

export default {
  title: "Summer lucky 12",
  items: [
    { text: "Watch the sunrise", revealed: true, done: true },
    { text: "Road trip with zero plan", revealed: true, done: false },
    fresh("Learn a TikTok dance"),
    fresh("Midnight swim"),
    fresh("Try a cuisine you can’t pronounce"),
    fresh("Sunset picnic"),
    fresh("Write a letter to future me"),
    fresh("$15 thrift challenge"),
    fresh("Stargaze somewhere dark"),
    fresh("Make friendship bracelets"),
    fresh("Dance in the rain"),
    fresh("Say yes to something scary"),
  ],
} satisfies Input<typeof schema.descriptor>;
