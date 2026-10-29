import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "contact-card",
  title: "Contact Card",
  description: "A translucent mint glass badge and digital business card with direct links, bio, and vCard export.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["personal", "utilities"],
  window: { kind: "standard", width: 440, height: 580 },
  theme: {
    surface: "#1c352a",
    canvas: "#182d24",
    glass: "#def3ed",
    glassSubtle: "#ebf7f3",
    border: "#9ecbbe",
    borderStrong: "#78b2a1",
    ink: "#113124",
    muted: "#496d5e",
    dim: "#7b9e90",
    pill: "#ffffff",
    pillHover: "#f4faf8",
    accent: "#0f766e",
    onAccent: "#ffffff",
    avatarFill: "#a7d5c7",
    avatarInk: "#254d3d",
  },
});
