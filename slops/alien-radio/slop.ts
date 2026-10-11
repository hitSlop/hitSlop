import { defineSlop } from "hitslop";
import skin from "./assets/window-mask.png";

export default defineSlop({
  slug: "alien-radio",
  title: "Alien Radio",
  description: "A sculpted desktop receiver for exploring listener-supported SomaFM stations.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["media", "personal"],
  window: { kind: "skin", width: 720, height: 560, image: skin },
  theme: {
    ink: "#eaffb3",
    lime: "#caff42",
    limeSoft: "#e6ff9d",
    limeDim: "#779a2b",
    violet: "#aa7ce0",
    violetBright: "#b47aff",
    screen: "#0a0d07",
    plate: "#16190c",
  },
});
