import { defineTiny } from "@hitslop/tinys";

export default defineTiny({
  slug: "project-pan",
  title: "Project Pan",
  description: "A rose-gold compact for using up your makeup and skincare before buying more: watch each pan hollow out, track expiry, and shelve the empties.",
  author: { name: "tinys", url: "https://tinys.click" },
  categories: ["personal", "other"],
  window: { kind: "standard", width: 480, height: 740 },
  theme: {
    desk: "#2a1d22",
    shell: "#d9a79a",
    shellDeep: "#b3766a",
    shellLight: "#f1cdc2",
    glass: "#e8eef2",
    tray: "#33262b",
    metal: "#c9ccd2",
    ink: "#2f2227",
    muted: "#7d6a70",
    onTray: "#f6e9e4",
    accent: "#c2476b",
    warn: "#b8341f",
  },
});
