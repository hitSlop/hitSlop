import { defineDocument, s } from "hitslop";

export const stages = ["wishlist", "applied", "assessment", "interview", "offer", "rejected"] as const;

export default defineDocument({
  title: s.text(),
  applications: s.list(s.object({
    company: s.text(),
    role: s.text(),
    stage: s.enum(stages),
    followUp: s.optional(s.string({ maxLength: 10, description: "Local YYYY-MM-DD." })),
  })),
  chats: s.list(s.object({
    name: s.text(),
    place: s.text(),
    when: s.optional(s.string({ maxLength: 10, description: "Local YYYY-MM-DD." })),
    thanked: s.boolean({ default: false }),
  })),
}, { initial: {
  title: "Summer internships",
  applications: [
    { company: "Figma", role: "Product design intern", stage: "interview" },
    { company: "Spotify", role: "Data science intern", stage: "assessment" },
    { company: "Notion", role: "Software engineering intern", stage: "applied" },
    { company: "Patagonia", role: "Marketing intern", stage: "applied" },
    { company: "NPR", role: "Audience research intern", stage: "wishlist" },
    { company: "Stripe", role: "Software engineering intern", stage: "rejected" },
  ],
  chats: [
    { name: "Priya S.", place: "Alumni call · Figma", thanked: true },
    { name: "Marcus L.", place: "Coffee · Spotify", thanked: false },
  ],
} });
