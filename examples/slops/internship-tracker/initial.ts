import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
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
} satisfies Input<typeof schema.descriptor>;
