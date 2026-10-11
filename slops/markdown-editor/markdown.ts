import { refuse, type Tx } from "hitslop";
import type doc from "./schema";
import { modes } from "./schema";

type Edit = Tx<typeof doc>;
export type Data = typeof doc.current;
export type Mode = Data["mode"];
export type Theme = Data["theme"];
/** New text plus where the selection should land. */
export type Rewrite = { value: string; start: number; end: number };

/** The editor footer and the export derive their numbers from the same snapshot. */
export function markdownView(data: Data) {
  const trimmed = data.content.trim();
  const words = trimmed === "" ? 0 : trimmed.split(/\s+/).length;
  return {
    words,
    chars: data.content.length,
    readingTime: Math.max(1, Math.ceil(words / 200)),
    blank: trimmed === "",
    title: data.title.trim() || "Untitled",
  };
}

export function nextMode(mode: Mode): Mode {
  return modes[(modes.indexOf(mode) + 1) % modes.length]!;
}

// Pure text rewrites for the toolbar: the page turns the result into one setContent edit.
export function wrapSelection(value: string, start: number, end: number, marker: string): Rewrite {
  const selected = value.slice(start, end);
  if (selected.startsWith(marker) && selected.endsWith(marker) && selected.length >= marker.length * 2) {
    const inner = selected.slice(marker.length, selected.length - marker.length);
    return { value: value.slice(0, start) + inner + value.slice(end), start, end: start + inner.length };
  }
  if (value.slice(Math.max(0, start - marker.length), start) === marker && value.slice(end, end + marker.length) === marker) {
    return {
      value: value.slice(0, start - marker.length) + selected + value.slice(end + marker.length),
      start: start - marker.length,
      end: end - marker.length,
    };
  }
  const inner = selected || "text";
  return {
    value: value.slice(0, start) + marker + inner + marker + value.slice(end),
    start: start + marker.length,
    end: start + marker.length + inner.length,
  };
}
export function prefixLine(value: string, caret: number, prefix: string): Rewrite {
  const lineStart = value.lastIndexOf("\n", caret - 1) + 1;
  const lineEndIdx = value.indexOf("\n", caret);
  const lineEnd = lineEndIdx === -1 ? value.length : lineEndIdx;
  const line = value.slice(lineStart, lineEnd);
  const structural = /^(#{1,6} |> |[-*+] |\d+\. )/;
  let nextLine: string;
  if (line.startsWith(prefix)) nextLine = line.slice(prefix.length);
  else if (structural.test(line)) nextLine = line.replace(structural, prefix);
  else nextLine = prefix + line;
  const cursor = lineStart + nextLine.length;
  return { value: value.slice(0, lineStart) + nextLine + value.slice(lineEnd), start: cursor, end: cursor };
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function setTitle(tx: Edit, title: string) {
  tx.fields.title.set(title);
}
export function setContent(tx: Edit, content: string) {
  tx.fields.content.set(content);
}
export function appendContent(tx: Edit, current: Data, text: string) {
  if (!text) refuse("Enter some text to append.");
  const base = current.content;
  const joiner = base === "" || base.endsWith("\n") ? "" : "\n";
  tx.fields.content.set(base + joiner + text);
  return base.length + joiner.length + text.length;
}
export function setMode(tx: Edit, mode: Mode) {
  tx.fields.mode.set(mode);
}
export function setTheme(tx: Edit, theme: Theme) {
  tx.fields.theme.set(theme);
}
export function toggleTheme(tx: Edit, current: Data) {
  const theme: Theme = current.theme === "paper" ? "dark" : "paper";
  tx.fields.theme.set(theme);
  return theme;
}
