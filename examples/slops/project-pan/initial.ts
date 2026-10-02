import type { Input } from "@hitslop/document";
import schema from "./schema";

export default {
  title: "Project Pan",
  products: [
    { name: "Soft Pinch Blush", brand: "Rare Beauty", kind: "cheek", shade: "#e8a0a8", left: 34, pao: 24 },
    { name: "Hyper Gloss", brand: "Glossier", kind: "lips", shade: "#d96f7e", left: 62, pao: 18 },
    { name: "Dream Wonder Fluid", brand: "Maybelline", kind: "face", shade: "#d9b18f", left: 12, pao: 12 },
    { name: "Warm Neutrals Palette", brand: "Colourpop", kind: "eyes", shade: "#a8704f", left: 81, pao: 36 },
    { name: "Lash Sensational", brand: "Maybelline", kind: "eyes", shade: "#7b3f5e", left: 45, pao: 6 },
    { name: "Cica Balm", brand: "Dr. Jart+", kind: "skin", shade: "#8fa58a", left: 71, pao: 12 },
    { name: "Cloud Paint", brand: "Glossier", kind: "cheek", shade: "#e9a07b", left: 0, pao: 18 },
    { name: "Lip Sleeping Mask", brand: "Laneige", kind: "lips", shade: "#b8445f", left: 0, pao: 12 },
  ],
} satisfies Input<typeof schema.descriptor>;
