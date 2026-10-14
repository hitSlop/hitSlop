import { defineDocument, s } from "hitslop";

export default defineDocument({
  title: s.text(),
  doneLaneKey: s.optional(s.string()),
  lanes: s.list(s.object({
    laneKey: s.string(),
    title: s.text(),
    limit: s.optional(s.integer({ min: 0, max: 999 })),
  })),
  cards: s.list(s.object({
    laneKey: s.string(),
    title: s.text(),
    note: s.text(),
    tag: s.text(),
  })),
}, { initial: {
  title: "Work Order Board",
  doneLaneKey: "shipped",
  lanes: [
    { laneKey: "queued", title: "Queued" },
    { laneKey: "running", title: "Running", limit: 3 },
    { laneKey: "review", title: "Review", limit: 2 },
    { laneKey: "shipped", title: "Shipped" },
  ],
  cards: [
    { laneKey: "queued", title: "Rebuild the spindle jig", note: "Waiting on the 12mm collet before setup can start.", tag: "shop" },
    { laneKey: "queued", title: "Price the spring catalog", note: "Compare last season's margins first.", tag: "admin" },
    { laneKey: "running", title: "Anodize the panel batch", note: "Bath is up to temperature; 40 pieces on the rack.", tag: "finish" },
    { laneKey: "running", title: "Loom the control wiring", note: "", tag: "assembly" },
    { laneKey: "review", title: "Check the gauge tolerances", note: "Two pieces sit 0.04mm over. Decide scrap or rework.", tag: "qa" },
    { laneKey: "shipped", title: "Crate the Hutton order", note: "Left on the Thursday truck.", tag: "logistics" },
  ],
} });
