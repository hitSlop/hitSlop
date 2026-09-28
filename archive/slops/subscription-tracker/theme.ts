import { defineTheme } from "@hitslop/document/theme";

export default defineTheme({
  surface: "#f4f3fa",
  paper: "#ffffff",
  paperSoft: "#efecfa",
  ink: "#242039",
  muted: "#696378",
  rule: "color-mix(in srgb, var(--slop-ink) 16%, transparent)",
  accent: "#6740c8",
  accentInk: "#292040",
  warning: "#825310",
  danger: "#b33655",
  paused: "#746c84",
  mint: "#b6f3d8",
  onDark: "#ffffff",
  mutedOnDark: "#c6bed9",
  headingFont: '"Subscription Space", "Avenir Next", sans-serif',
  font: '"Avenir Next", Avenir, "Helvetica Neue", sans-serif',
});
