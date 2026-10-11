import { defineSlop } from "hitslop";

export default defineSlop({
  slug: "internship-tracker",
  title: "Internship Tracker",
  description: "A conference name badge for tracking internship and job applications from wishlist to offer, with follow-up dates and a coffee-chat log.",
  author: { name: "hitSlop", url: "https://hitslop.com" },
  categories: ["personal", "productivity"],
  window: { kind: "standard", width: 480, height: 700 },
  theme: {
    strap: "#16213e",
    strapEdge: "#0d1428",
    badge: "#fbfaf6",
    ink: "#1b1d2a",
    muted: "#6a6d7c",
    rule: "#1b1d2a1f",
    band: "#e5432d",
    onBand: "#ffffff",
    wishlist: "#5b6b8c",
    applied: "#2563eb",
    assessment: "#7c3aed",
    interview: "#d97706",
    offer: "#15803d",
    rejected: "#b4232a",
  },
});
