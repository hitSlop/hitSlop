import { Img, interpolate, staticFile, useCurrentFrame } from "remotion";
import { mono, sans } from "./Desktop";
import type { Point } from "./timeline";

export const FINDER = { x: 96, y: 92, w: 820, h: 640, cols: 4, cellW: 190, cellH: 176, top: 104, left: 30 };
export const finderFiles = [
  ["soma-amp", "soma-amp.tiny"],
  ["codex-pet", "codex-pet.tiny"],
  ["focus-timer", "focus-timer.tiny"],
  ["quick-checklist-fast", "Finals week.tiny"],
  ["doodle-board", "doodle-board.tiny"],
  ["koi-pond", "koi-pond.tiny"],
  ["flashcards", "flashcards.tiny"],
  ["school-schedule", "school-schedule.tiny"],
  ["wordle", "wordle.tiny"],
  ["kanban-board", "kanban-board.tiny"],
  ["side-quest", "side-quest.tiny"],
  ["pixel-art", "pixel-art.tiny"],
] as const;

export function finderIcon(i: number): Point {
  const col = i % FINDER.cols, row = Math.floor(i / FINDER.cols);
  return { x: FINDER.x + FINDER.left + col * FINDER.cellW + FINDER.cellW / 2, y: FINDER.y + FINDER.top + row * FINDER.cellH + 62 };
}

function Lights() {
  return (
    <div style={{ display: "flex", gap: 10 }}>
      {["#ff5f57", "#febc2e", "#28c840"].map((c) => <div key={c} style={{ width: 16, height: 16, borderRadius: 9, background: c, boxShadow: "inset 0 0 0 .5px #0003" }} />)}
    </div>
  );
}

export function Finder({ selected, selectedAt }: { selected: number[]; selectedAt: number[] }) {
  const frame = useCurrentFrame();
  return (
    <div style={{ width: "100%", height: "100%", background: "rgba(250,249,255,.93)", backdropFilter: "blur(40px)", fontFamily: sans, color: "#1d1b2e" }}>
      <div style={{ height: 76, display: "flex", alignItems: "center", gap: 26, padding: "0 22px", borderBottom: "1px solid #0000000f" }}>
        <Lights />
        <span style={{ fontSize: 22, color: "#8c89a3" }}>‹ ›</span>
        <span style={{ fontSize: 21, fontWeight: 700 }}>tinys</span>
        <span style={{ marginLeft: "auto", fontSize: 16, color: "#8c89a3" }}>{finderFiles.length} items</span>
      </div>
      <div style={{ position: "relative" }}>
        {finderFiles.map(([icon, label], i) => {
          const p = finderIcon(i);
          const on = selected.some((s, k) => s === i && frame >= selectedAt[k] && (selectedAt[k + 1] === undefined || frame < selectedAt[k + 1]));
          const pop = interpolate(frame, [20 + i * 1.5, 34 + i * 1.5], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
          return (
            <div key={icon} style={{ position: "absolute", left: p.x - FINDER.x - FINDER.cellW / 2, top: p.y - FINDER.y - 62 - 76, width: FINDER.cellW, display: "flex", flexDirection: "column", alignItems: "center", gap: 8, opacity: pop, transform: `translateY(${(1 - pop) * 14}px)` }}>
              <div style={{ padding: 8, borderRadius: 16, background: on ? "#0000001a" : "transparent" }}>
                <Img src={staticFile(`icons/${icon}.png`)} style={{ width: 100, height: 100, display: "block" }} />
              </div>
              <span style={{ fontSize: 16, fontWeight: 500, padding: "2px 8px", borderRadius: 6, background: on ? "#5b5bd6" : "transparent", color: on ? "#fff" : "#1d1b2e" }}>{label}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

export type Command = { start: number; enter: number; text: string; output: string[] };

export function Terminal({ commands }: { commands: Command[] }) {
  const frame = useCurrentFrame();
  const lines: { text: string; kind: "cmd" | "out" }[] = [];
  let cursorVisible = true;
  for (const c of commands) {
    if (frame < c.start) break;
    const typed = Math.floor(interpolate(frame, [c.start, c.enter - 4], [0, c.text.length], { extrapolateRight: "clamp" }));
    lines.push({ text: c.text.slice(0, typed), kind: "cmd" });
    if (frame >= c.enter) {
      const shown = Math.floor((frame - c.enter) / 1.2);
      c.output.slice(0, shown).forEach((text) => lines.push({ text, kind: "out" }));
      cursorVisible = false;
    } else cursorVisible = true;
  }
  const blink = Math.floor(frame / 15) % 2 === 0;
  return (
    <div style={{ width: "100%", height: "100%", background: "rgba(22,20,34,.94)", fontFamily: mono, fontSize: 19, color: "#e8e6f5", display: "flex", flexDirection: "column" }}>
      <div style={{ height: 50, display: "flex", alignItems: "center", padding: "0 18px", gap: 16, borderBottom: "1px solid #ffffff14" }}>
        <Lights />
        <span style={{ flex: 1, textAlign: "center", fontFamily: sans, fontSize: 16, color: "#a9a5c4", marginRight: 70 }}>~/Desktop/tinys — zsh</span>
      </div>
      <div style={{ padding: "16px 22px", lineHeight: 1.5, display: "flex", flexDirection: "column", justifyContent: "flex-end", flex: 1, overflow: "hidden" }}>
        {lines.map((l, i) => (
          <div key={i} style={{ whiteSpace: "pre-wrap", wordBreak: "break-all", color: l.kind === "out" ? "#9d98c0" : "#f3f1ff" }}>
            {l.kind === "cmd" && <span style={{ color: "#8bf0c7" }}>❯ </span>}
            {l.kind === "cmd" ? highlight(l.text) : l.text}
            {i === lines.length - 1 && l.kind === "cmd" && cursorVisible && <span style={{ opacity: blink ? 1 : 0, background: "#f3f1ff" }}>&nbsp;</span>}
          </div>
        ))}
      </div>
    </div>
  );
}

function highlight(text: string) {
  const [cmd, ...rest] = text.split(" ");
  return <><span style={{ color: "#ff9bd2", fontWeight: 700 }}>{cmd}</span>{rest.length ? " " + rest.join(" ") : ""}</>;
}
