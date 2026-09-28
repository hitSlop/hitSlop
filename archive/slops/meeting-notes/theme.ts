import { defineTheme } from "@hitslop/document/theme";

export default defineTheme({
  surface: "#faf8f2",
  paper: "#faf8f2",
  paperSoft: "#f2efe7",
  letterhead: "#24362a",
  letterheadInk: "#f5f4ef",
  ink: "#1a251e",
  muted: "#606d64",
  dim: "#9aa79e",
  rule: "color-mix(in srgb, var(--slop-ink) 14%, transparent)",
  accent: "#c4dc33",
  accentInk: "#2b3908",
  accentSoft: "rgba(196, 220, 51, 0.18)",
  decision: "#f4f6e8",
  decisionRule: "#bdd33a",
  owner: "#e6ede6",
  ownerInk: "#233b2a",
  danger: "#a53a31",
  font: '"Avenir Next", Avenir, "Helvetica Neue", sans-serif',
  headingFont: 'Georgia, "Times New Roman", serif',
});
