import { defineDocument, s } from "hitslop";

export default defineDocument({
  title: s.text(),
  items: s.list(s.object({
    text: s.text(),
    revealed: s.boolean({ default: false }),
    done: s.boolean({ default: false }),
  })),
}, { initial: {
  title: "Summer lucky 12",
  items: [
    { text: "Watch the sunrise", revealed: true, done: true },
    { text: "Road trip with zero plan", revealed: true },
    { text: "Learn a TikTok dance" },
    { text: "Midnight swim" },
    { text: "Try a cuisine you can’t pronounce" },
    { text: "Sunset picnic" },
    { text: "Write a letter to future me" },
    { text: "$15 thrift challenge" },
    { text: "Stargaze somewhere dark" },
    { text: "Make friendship bracelets" },
    { text: "Dance in the rain" },
    { text: "Say yes to something scary" },
  ],
} });
