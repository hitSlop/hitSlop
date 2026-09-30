import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  deckTitle: "Single-File Desktop Objects",
  theme: "swiss",
  activeSlideIndex: 0,
  slides: [
    {
      layout: "title",
      title: "Autonomous Desktop Objects",
      subtitle: "The unreasonable effectiveness of single-file local-first software.",
      tag: "LONGTAIL LABS · 2026",
      points: [],
      cards: [],
      notes: "Welcome everyone. Introduce the thesis of durable single-file software.",
    },
    {
      layout: "split",
      title: "Why Single-File Matters",
      points: [
        "Zero cloud rent — you own the file forever",
        "Instant offline execution in any modern browser",
        "Plain view-source honest JSON data",
        "No SaaS logins, subscriptions, or dark patterns",
      ],
      highlightLabel: "DURABILITY",
      highlightValue: "10+ Years",
      highlightDesc: "A file saved in 2026 will open without servers in 2036.",
      cards: [],
      notes: "Emphasize data sovereignty and local longevity.",
    },
    {
      layout: "metric",
      title: "Adoption Velocity",
      metricValue: "12.4x",
      metricLabel: "FASTER TIME-TO-VALUE",
      subtitle: "Teams craft and share bespoke instruments in minutes rather than quarters.",
      tag: "MEASURED OUTCOMES",
      points: [],
      cards: [],
      notes: "Highlight rapid iteration cycles compared to bloated enterprise apps.",
    },
    {
      layout: "quote",
      quoteText: "The interface should feel composed before it feels configurable. Start with a strong opinion, then leave the edges open for the owner.",
      author: "hitSlop Principles",
      points: [],
      cards: [],
      notes: "Pause on this quote to reinforce the craft-first ethos.",
    },
    {
      layout: "cards",
      title: "Three Core Tenets",
      points: [],
      cards: [
        { title: "Tactile Objects", desc: "Apps designed as physical instruments on your desk." },
        { title: "Host Sovereignty", desc: "Data belongs to the user, not a remote server database." },
        { title: "Zero Drag", desc: "Instant boot, offline-first, under 100KB bundles." },
      ],
      notes: "Wrap up key pillars and open the floor to Q&A.",
    },
  ],
} satisfies Input<typeof schema.fields.node>;
