import { defineTheme } from "@hitslop/document/theme";

export default defineTheme({
  surface: "#edf1ff",
  paper: "#ffffff",
  paperSoft: "#f1f4ff",
  ink: "#182338",
  muted: "#586279",
  rule: "color-mix(in srgb, var(--slop-ink) 16%, transparent)",
  accent: "#2448c8",
  accentInk: "#ffffff",
  accentSoft: "#e7edff",
  herb: "#c7d4ff",
  onAccent: "#ffffff",
  cook: "#142452",
  cookSoft: "#223665",
  cookMuted: "#c5cee5",
  font: '"Avenir Next", Avenir, "Helvetica Neue", sans-serif',
  headingFont: '"Recipe Fraunces", Georgia, serif',
  tomato: "#c83c29",
  mono: 'ui-monospace, "SFMono-Regular", Menlo, monospace',
});
