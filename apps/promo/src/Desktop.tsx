import type { CSSProperties, ReactNode } from "react";
import { AbsoluteFill, Easing, Img, interpolate, spring, staticFile, useCurrentFrame, useVideoConfig } from "remotion";
import { UI, type Point } from "./timeline";

export const fontFaces = `
@font-face { font-family: "Lilita One"; src: url(${staticFile("fonts/lilita-one-latin.woff2")}) format("woff2"); }
@font-face { font-family: "DM Sans"; src: url(${staticFile("fonts/dm-sans-latin.woff2")}) format("woff2"); font-weight: 400 700; }
@font-face { font-family: "Kalam"; src: url(${staticFile("fonts/kalam-latin.woff2")}) format("woff2"); }
`;
export const sans = `"DM Sans", -apple-system, "SF Pro Text", "Helvetica Neue", sans-serif`;
export const mono = `"SF Mono", Menlo, ui-monospace, monospace`;

export function Wallpaper() {
  const frame = useCurrentFrame();
  const drift = Math.sin(frame / 90) * 30;
  return (
    <AbsoluteFill
      style={{
        background: `radial-gradient(ellipse 60% 70% at ${22 + drift / 20}% 28%, #f7b3d6 0, transparent 60%),
          radial-gradient(ellipse 55% 60% at ${78 - drift / 25}% 22%, #b9b1ff 0, transparent 62%),
          radial-gradient(ellipse 70% 60% at 70% 92%, #8fe3dc 0, transparent 60%),
          radial-gradient(ellipse 60% 55% at 12% 90%, #ffd49a 0, transparent 60%),
          linear-gradient(160deg, #6f63d9, #4b3fb3 55%, #2f2a7d)`,
      }}
    />
  );
}

export function MenuBar({ app = "Finder" }: { app?: string }) {
  const h = 26 * UI;
  const item: CSSProperties = { fontSize: 13.5 * UI, fontWeight: 500, color: "#1d1b2e" };
  return (
    <div
      style={{
        position: "absolute", inset: "0 0 auto 0", height: h, display: "flex", alignItems: "center", gap: 22 * UI,
        padding: `0 ${14 * UI}px`, background: "rgba(255,255,255,.42)", backdropFilter: "blur(30px) saturate(1.6)",
        fontFamily: sans, zIndex: 50,
      }}
    >
      <svg width={15 * UI} height={15 * UI} viewBox="0 0 24 24"><circle cx="12" cy="12" r="9" fill="#1d1b2e" /><circle cx="9" cy="10" r="1.6" fill="#fff" /><circle cx="15" cy="10" r="1.6" fill="#fff" /><path d="M8 14.5q4 3.5 8 0" stroke="#fff" strokeWidth="1.8" fill="none" strokeLinecap="round" /></svg>
      <span style={{ ...item, fontWeight: 700 }}>{app}</span>
      {["File", "Edit", "View", "Window", "Help"].map((m) => <span key={m} style={item}>{m}</span>)}
      <span style={{ ...item, marginLeft: "auto" }}>Sat Sep 26&nbsp;&nbsp;10:15 AM</span>
    </div>
  );
}

export function FolderGlyph({ size }: { size: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 64 64">
      <defs><linearGradient id="fg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stopColor="#8fd0ff" /><stop offset="1" stopColor="#3f97e8" /></linearGradient></defs>
      <path d="M6 16a5 5 0 0 1 5-5h14l5 5h23a5 5 0 0 1 5 5v28a5 5 0 0 1-5 5H11a5 5 0 0 1-5-5Z" fill="#3a86d6" />
      <path d="M6 23a5 5 0 0 1 5-5h42a5 5 0 0 1 5 5v26a5 5 0 0 1-5 5H11a5 5 0 0 1-5-5Z" fill="url(#fg)" />
    </svg>
  );
}

function TerminalGlyph({ size }: { size: number }) {
  return (
    <div style={{ width: size, height: size, borderRadius: size * 0.22, background: "linear-gradient(#3b3d4a,#17181f)", boxShadow: "inset 0 0 0 1.5px #ffffff30", display: "grid", placeItems: "center" }}>
      <span style={{ fontFamily: mono, color: "#e9e9ee", fontSize: size * 0.34, fontWeight: 700 }}>&gt;_</span>
    </div>
  );
}

export const DOCK = { y: 1080 - 14, size: 66 * UI };
export const dockItems = ["finder", "tinys", "terminal"] as const;
export function dockPoint(i: number): Point {
  const gap = 10 * UI;
  const total = dockItems.length * DOCK.size + (dockItems.length - 1) * gap;
  return { x: 960 - total / 2 + i * (DOCK.size + gap) + DOCK.size / 2, y: DOCK.y - 10 * UI - DOCK.size / 2 };
}

export function Dock({ running, bounce }: { running: string[]; bounce?: { item: string; at: number } }) {
  const frame = useCurrentFrame();
  const pad = 10 * UI;
  return (
    <div
      style={{
        position: "absolute", left: "50%", bottom: 14, transform: "translateX(-50%)", display: "flex", gap: 10 * UI,
        padding: `${pad}px ${pad * 1.2}px ${pad}px`, borderRadius: 26 * UI, background: "rgba(255,255,255,.28)",
        border: "1px solid rgba(255,255,255,.45)", backdropFilter: "blur(30px) saturate(1.5)", zIndex: 40,
      }}
    >
      {dockItems.map((item) => {
        const t = bounce?.item === item ? frame - bounce.at : -1;
        const lift = t >= 0 && t < 24 ? Math.abs(Math.sin((t / 24) * Math.PI * 2)) * 22 * (1 - t / 24) : 0;
        return (
          <div key={item} style={{ position: "relative", width: DOCK.size, height: DOCK.size, transform: `translateY(${-lift}px)` }}>
            {item === "finder" && <FolderGlyph size={DOCK.size} />}
            {item === "tinys" && <Img src={staticFile("icons/tinys-app.png")} style={{ width: "100%", height: "100%" }} />}
            {item === "terminal" && <TerminalGlyph size={DOCK.size} />}
            {running.includes(item) && <div style={{ position: "absolute", bottom: -7 * UI, left: "50%", width: 5, height: 5, marginLeft: -2.5, borderRadius: 9, background: "#1d1b2e" }} />}
          </div>
        );
      })}
    </div>
  );
}

/** Springs a window open from `origin` (icon) to its resting frame; optionally closes. */
export function useOpen(at: number, closeAt?: number) {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const open = spring({ frame: frame - at, fps, config: { damping: 15, mass: 0.7, stiffness: 140 } });
  const close = closeAt === undefined ? 0 : interpolate(frame, [closeAt, closeAt + 10], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: Easing.in(Easing.cubic) });
  return { progress: open, close, visible: frame >= at && close < 1 };
}

export function TinyWindow({
  x, y, width, height, scale = 1, shape = "rounded", at, closeAt, origin, z = 10, children, glow,
}: {
  x: number; y: number; width: number; height: number; scale?: number; shape?: "rounded" | "ellipse" | "none";
  at: number; closeAt?: number; origin?: Point; z?: number; children: ReactNode; glow?: number;
}) {
  const { progress, close, visible } = useOpen(at, closeAt);
  if (!visible) return null;
  const w = width * scale, h = height * scale;
  const from = origin ?? { x: x + w / 2, y: y + h / 2 + 40 };
  const cx = interpolate(progress, [0, 1], [from.x - (x + w / 2), 0]);
  const cy = interpolate(progress, [0, 1], [from.y - (y + h / 2), 0]);
  const s = interpolate(progress, [0, 1], [0.08, 1]) * (1 - close * 0.12);
  const radius = shape === "ellipse" ? "50%" : shape === "rounded" ? 22 * scale : 0;
  return (
    <div
      style={{
        position: "absolute", left: x, top: y, width: w, height: h, zIndex: z,
        transform: `translate(${cx}px, ${cy}px) scale(${s})`, opacity: Math.min(1, progress * 3) * (1 - close),
        filter: shape === "none" ? `drop-shadow(0 ${14 * scale}px ${22 * scale}px rgba(20,10,60,.35))` : undefined,
      }}
    >
      <div
        style={{
          width: "100%", height: "100%", borderRadius: radius, overflow: shape === "none" ? "visible" : "hidden",
          boxShadow: shape === "none" ? undefined : `0 ${22 * scale}px ${60 * scale}px rgba(25,12,70,.42), 0 0 0 1px rgba(0,0,0,.18)${glow ? `, 0 0 0 ${glow}px rgba(255,255,255,.85)` : ""}`,
        }}
      >
        {children}
      </div>
    </div>
  );
}

export function FileIcon({ label, kind, size = 64 * UI }: { label: string; kind: "wsz" | "zip"; size?: number }) {
  const color = kind === "wsz" ? "#f48fb1" : "#ffd166";
  return (
    <div style={{ width: size * 1.9, display: "flex", flexDirection: "column", alignItems: "center", gap: 6 }}>
      <svg width={size} height={size} viewBox="0 0 64 64" style={{ filter: "drop-shadow(0 4px 6px rgba(0,0,0,.25))" }}>
        <path d="M14 4h26l12 12v42a2 2 0 0 1-2 2H14a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2Z" fill="#fff" />
        <path d="M40 4v10a2 2 0 0 0 2 2h10" fill="#e6e6ef" />
        {kind === "zip" ? (
          <><rect x="28" y="10" width="6" height="30" fill="#c9c9d6" /><rect x="26" y="34" width="10" height="10" rx="2" fill="#8a8aa0" /></>
        ) : (
          <><rect x="18" y="26" width="28" height="16" rx="2" fill="#39365f" /><rect x="21" y="29" width="14" height="5" fill="#71f06a" /><rect x="21" y="36" width="22" height="3" fill="#ffd166" /></>
        )}
        <rect x="16" y="48" width="32" height="8" rx="4" fill={color} />
        <text x="32" y="54.4" fontSize="6.4" textAnchor="middle" fontFamily="Helvetica" fontWeight="700" fill="#2a2640">{kind.toUpperCase()}</text>
      </svg>
      <span style={{ fontFamily: sans, fontSize: 12 * UI, fontWeight: 600, color: "#fff", textAlign: "center", lineHeight: 1.15, textShadow: "0 1px 3px rgba(0,0,0,.6)" }}>{label}</span>
    </div>
  );
}

/** Mac-like arrow cursor with a click ripple. */
export function Cursor({ at, clicks }: { at: Point; clicks: number[] }) {
  const frame = useCurrentFrame();
  const since = clicks.map((c) => frame - c).filter((d) => d >= 0 && d < 14);
  const pressed = clicks.some((c) => frame - c >= 0 && frame - c < 4);
  return (
    <div style={{ position: "absolute", left: at.x, top: at.y, zIndex: 1000, pointerEvents: "none" }}>
      {since.map((d, i) => (
        <div key={i} style={{ position: "absolute", left: -28, top: -28, width: 56, height: 56, borderRadius: 99, border: "3px solid rgba(255,255,255,.9)", transform: `scale(${0.3 + d / 12})`, opacity: 1 - d / 14 }} />
      ))}
      <svg width={30 * UI * 0.9} height={30 * UI * 0.9} viewBox="0 0 24 24" style={{ transform: `translate(-3px,-2px) scale(${pressed ? 0.86 : 1})`, transformOrigin: "0 0", filter: "drop-shadow(0 2px 3px rgba(0,0,0,.35))" }}>
        <path d="M3 2v18l5-5 3.5 7.5 3-1.4L11 13.8h7Z" fill="#111" stroke="#fff" strokeWidth="1.6" strokeLinejoin="round" />
      </svg>
    </div>
  );
}

export type Key = { f: number; x: number; y: number };
export function cursorAt(frame: number, keys: Key[]): Point {
  if (frame <= keys[0].f) return keys[0];
  for (let i = 1; i < keys.length; i++) {
    const a = keys[i - 1], b = keys[i];
    if (frame <= b.f) {
      const t = Easing.bezier(0.45, 0, 0.2, 1)((frame - a.f) / Math.max(1, b.f - a.f));
      // Slight arc so moves feel hand-driven.
      const arc = Math.sin(t * Math.PI) * Math.min(60, Math.hypot(b.x - a.x, b.y - a.y) * 0.08);
      return { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t - arc };
    }
  }
  return keys.at(-1)!;
}
