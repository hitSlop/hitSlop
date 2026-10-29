import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "five-minute-journal",
  title: "Five Minute Journal",
  description: "A paper morning-and-evening bookend journal for three gratitudes, intentions, an affirmation, and dusk reflections.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["personal", "productivity"],
  window: { kind: "standard", width: 560, height: 780 },
  theme: {
    surface: "#f1eadc",
    paper: "#faf6ec",
    ink: "#2a241c",
    muted: "#726554",
    dim: "#796c5b",
    morning: "#f7ead0",
    morningDeep: "#eed7a7",
    morningInk: "#6b4a12",
    morningAccent: "#ba843b",
    evening: "#e9ebee",
    eveningDeep: "#dce1e8",
    eveningInk: "#374959",
    eveningAccent: "#475d73",
  },
});
