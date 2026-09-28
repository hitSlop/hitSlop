import { defineTheme } from "@hitslop/document/theme";

export default defineTheme({
  surface: "#f1eadc",
  paper: "#faf6ec",
  ink: "#2a241c",
  muted: "#726554",
  dim: "#796c5b",
  rule: "color-mix(in srgb, var(--slop-ink) 14%, transparent)",
  morning: "#f7ead0",
  morningDeep: "#eed7a7",
  morningInk: "#6b4a12",
  morningAccent: "#ba843b",
  evening: "#e9ebee",
  eveningDeep: "#dce1e8",
  eveningInk: "#374959",
  eveningAccent: "#475d73",
  font: "-apple-system, BlinkMacSystemFont, \"Segoe UI\", Roboto, Helvetica, Arial, sans-serif",
  headingFont: "\"Journal Lora\", Georgia, \"Times New Roman\", serif",
  mono: "ui-monospace, SFMono-Regular, Menlo, monospace",
});
