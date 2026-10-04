import { AbsoluteFill, Audio, Img, interpolate, Sequence, spring, staticFile, useCurrentFrame, useVideoConfig } from "remotion";
import { Clip, clips } from "./Clip";
import { Cursor, cursorAt, dockPoint, Dock, FileIcon, fontFaces, MenuBar, sans, SlopWindow, Wallpaper, type Key } from "./Desktop";
import { FINDER, Finder, finderIcon, Terminal, type Command } from "./Windows";
import { DURATION, MUSIC, popBeats, punchBeats, T, type Point } from "./timeline";

// Resting frames (composition px). Captures are CSS-size × 2, drawn at `scale`.
const SOMA = { x: 1010, y: 120, w: 275, h: 470, scale: 1.3 };
const PET = { x: 1450, y: 640, w: 240, h: 180, scale: 1.4 };
const CHECK = { x: 110, y: 64, w: 480, h: 620, scale: 1 };
const TIMER = { x: 640, y: 104, w: 320, h: 320, scale: 1 };
const TERM = { x: 360, y: 610, w: 920, h: 360 };
const WSZ = { x: 1700, y: 70 };
const ZIP = { x: 1700, y: 270 };
const fileCenter = (p: Point) => ({ x: p.x + 79, y: p.y + 42 });
const center = (r: { x: number; y: number; w: number; h: number; scale: number }) => ({ x: r.x + (r.w * r.scale) / 2, y: r.y + (r.h * r.scale) / 2 });

const dbl = (f: number) => [f, f + 5];
const OPEN = { soma: T.somaOpen, pet: T.petOpen, checklist: T.checkOpen, timer: T.timerOpen };
const CHECK_DOUBLE = T.checkDouble;
const TIMER_DOUBLE = T.timerDouble;
const TERM_CLICK = T.termClick;
// Drops land two frames before the swap they trigger.
const WSZ_DROP = T.skinSwap - 2;
const ZIP_DROP = T.petSwap - 2;

const somaPlayButton = { x: SOMA.x + 50 * SOMA.scale, y: SOMA.y + 96 * SOMA.scale };
const cursorKeys: Key[] = [
  { f: 0, x: 980, y: 600 },
  { f: 32, ...dockPoint(0) },
  { f: 60, ...dockPoint(0) },
  { f: T.somaDouble - 2, ...finderIcon(0) },
  { f: T.somaPlay - 4, ...somaPlayButton },
  { f: T.wszGrab - 4, ...fileCenter(WSZ) },
  { f: WSZ_DROP - 4, ...center(SOMA) },
  { f: WSZ_DROP + 14, ...center(SOMA) },
  { f: T.petDouble - 2, ...finderIcon(1) },
  { f: T.zipGrab - 4, ...fileCenter(ZIP) },
  { f: ZIP_DROP - 4, ...center(PET) },
  { f: ZIP_DROP + 6, ...center(PET) },
  { f: CHECK_DOUBLE - 2, ...finderIcon(3) },
  { f: TIMER_DOUBLE - 2, ...finderIcon(2) },
  { f: TERM_CLICK - 3, ...dockPoint(2) },
  { f: T.cmd1Start + 6, x: 1330, y: 560 },
  { f: T.termClose, x: 1330, y: 560 },
  { f: T.montage + 20, x: 1990, y: 1100 },
];
const clicks = [T.finderClick, ...dbl(T.somaDouble), T.somaPlay, T.wszGrab, ...dbl(T.petDouble), T.zipGrab, ...dbl(CHECK_DOUBLE), ...dbl(TIMER_DOUBLE), TERM_CLICK];

const m = (name: keyof typeof clips, mark: string) => (clips[name].marks as Record<string, number>)[mark];

const commands: Command[] = [
  {
    start: T.cmd1Start,
    enter: T.cmd1Enter,
    text: `slop apply "Finals week.slop" --op '{"type":"insert","path":["tasks"],"value":{"text":"Review bio flashcards","done":false,"archived":false}}'`,
    output: [`      "text" : "Review bio flashcards"`, `    }`, `  ],`, `  "title" : "Finals week"`, `}`],
  },
  {
    start: T.cmd2Start,
    enter: T.cmd2Enter,
    text: `slop theme set focus-timer.slop --values '{"surface":"#3b6fd4","surfaceLight":"#5b8ef0","surfaceDeep":"#284f9e","accent":"#2f5fc0"}'`,
    output: [`    "surface" : "#3b6fd4",`, `    "surfaceDeep" : "#284f9e",`, `    "surfaceLight" : "#5b8ef0"`, `  }`, `}`],
  },
];

type Pop = { at: number; x: number; y: number; w: number; h: number; scale: number; shape?: "rounded" | "ellipse" | "none" } & ({ clip: keyof typeof clips; loop?: boolean } | { still: string });
const montage: Pop[] = [
  { still: "kanban-board", at: popBeats[0] - 2, x: 40, y: 60, w: 880, h: 620, scale: 0.56 },
  { clip: "flashcards", at: popBeats[1] - 2, x: 1470, y: 56, w: 540, h: 440, scale: 0.74 },
  { clip: "doodle", at: popBeats[2] - 2, x: 560, y: 120, w: 744, h: 594, scale: 0.68 },
  { still: "school-schedule", at: popBeats[3] - 2, x: 40, y: 470, w: 880, h: 640, scale: 0.64 },
  { clip: "koi", at: popBeats[4] - 2, x: 1545, y: 270, w: 420, h: 420, scale: 0.84, shape: "none", loop: true },
  { clip: "wordle", at: popBeats[5] - 2, x: 1160, y: 380, w: 440, h: 620, scale: 0.64 },
  { still: "side-quest", at: popBeats[6] - 2, x: 620, y: 520, w: 940, h: 640, scale: 0.56 },
  { still: "pixel-art", at: popBeats[7] - 2, x: 1250, y: 60, w: 460, h: 620, scale: 0.5 },
];

function DraggedFile({ at, grab, drop, target, label, kind }: { at: Point; grab: number; drop: number; target: Point; label: string; kind: "wsz" | "zip" }) {
  const frame = useCurrentFrame();
  const cursor = cursorAt(frame, cursorKeys);
  const dragging = frame >= grab && frame < drop;
  const gone = interpolate(frame, [drop, drop + 8], [1, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  if (gone <= 0) return null;
  const pos = frame < grab ? at : dragging ? { x: cursor.x - 79, y: cursor.y - 42 } : { x: target.x - 79, y: target.y - 42 };
  return (
    <div style={{ position: "absolute", left: pos.x, top: pos.y, zIndex: dragging || frame >= drop ? 900 : 5, opacity: (dragging ? 0.85 : 1) * gone, transform: `scale(${(dragging ? 1.06 : 1) * (0.4 + 0.6 * gone)})` }}>
      <FileIcon label={label} kind={kind} />
    </div>
  );
}

function Outro() {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const veil = interpolate(frame, [T.outro, T.outro + 18], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  if (veil <= 0) return null;
  const pop = (d: number) => spring({ frame: frame - T.outro - d, fps, config: { damping: 12, mass: 0.7 } });
  return (
    <AbsoluteFill style={{ zIndex: 2000, alignItems: "center", justifyContent: "center", background: `radial-gradient(ellipse at 50% 45%, rgba(255,250,255,${0.93 * veil}) 0, rgba(246,240,255,${0.86 * veil}) 55%, rgba(236,228,255,${0.8 * veil}) 100%)`, backdropFilter: `blur(${veil * 14}px)` }}>
      <Img src={staticFile("icons/hitslop-app.png")} style={{ width: 190, height: 190, transform: `scale(${pop(15)}) rotate(${(1 - pop(15)) * -20}deg)`, filter: "drop-shadow(0 18px 30px rgba(60,30,140,.3))" }} />
      <div style={{ fontFamily: `"Lilita One", ${sans}`, fontSize: 150, color: "#10132c", lineHeight: 1, marginTop: 18, transform: `translateY(${(1 - pop(27)) * 40}px)`, opacity: pop(27) }}>hitSlop</div>
      <div style={{ fontFamily: sans, fontSize: 46, fontWeight: 600, color: "#3c3960", marginTop: 18, opacity: pop(39), transform: `translateY(${(1 - pop(39)) * 30}px)` }}>
        Tiny apps. <span style={{ color: "#f443a1" }}>Big</span> personality.
      </div>
      <div style={{ fontFamily: `"Kalam", cursive`, fontSize: 38, color: "#6c3fd6", marginTop: 22, opacity: pop(51), transform: `rotate(-3deg) scale(${0.8 + 0.2 * pop(51)})` }}>hitslop.com · free for Mac</div>
    </AbsoluteFill>
  );
}

export function Promo({ audio = true }: { audio?: boolean }) {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const cursor = cursorAt(frame, cursorKeys);
  // Stronger punch when a window pops, lighter on the beats between.
  const punch = punchBeats.reduce((sum, b) => sum + (frame >= b ? (popBeats.includes(b) ? 0.012 : 0.005) * Math.exp(-(frame - b) / 4) : 0), 0);
  const zoom = interpolate(frame, [T.montage, T.outro + 30], [1, 0.94], { extrapolateLeft: "clamp", extrapolateRight: "clamp" }) + punch;
  const dockRise = spring({ frame: frame - 4, fps, config: { damping: 16 } });
  const app = frame >= T.termOpen && frame < T.termClose ? "Terminal" : frame >= OPEN.soma ? "hitSlop" : "Finder";
  const running = ["finder", ...(frame >= OPEN.soma ? ["hitslop"] : []), ...(frame >= T.termOpen && frame < T.termClose + 6 ? ["terminal"] : [])];
  return (
    <AbsoluteFill style={{ overflow: "hidden", background: "#2f2a7d" }}>
      <style>{fontFaces}</style>
      {/* The song starts on SomaAmp's play press, hard on its kick; it fades out over the last second. */}
      {audio && (
        <Sequence from={T.somaPlay} layout="none">
          <Audio
            src={staticFile("music/new-york.mp3")}
            startFrom={Math.round(MUSIC.songStart * 30)}
            volume={(f) => interpolate(f + T.somaPlay, [DURATION - 30, DURATION - 1], [1, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp" })}
          />
        </Sequence>
      )}
      <Wallpaper />
      <AbsoluteFill style={{ transform: `scale(${zoom})` }}>
        <DraggedFile at={WSZ} grab={T.wszGrab} drop={WSZ_DROP} target={center(SOMA)} label="Tenchi Muyo - Aeka.wsz" kind="wsz" />
        <DraggedFile at={ZIP} grab={T.zipGrab} drop={ZIP_DROP} target={center(PET)} label="akitsuki-airi.codex-pet.zip" kind="zip" />

        <SlopWindow x={FINDER.x} y={FINDER.y} width={FINDER.w} height={FINDER.h} at={T.finderOpen} closeAt={TERM_CLICK} origin={dockPoint(0)} z={8}>
          <Finder selected={[0, 1, 3, 2]} selectedAt={[T.somaDouble, T.petDouble, CHECK_DOUBLE, TIMER_DOUBLE]} />
        </SlopWindow>

        <SlopWindow x={SOMA.x} y={SOMA.y} width={SOMA.w} height={SOMA.h} scale={SOMA.scale} at={OPEN.soma} origin={finderIcon(0)} z={12}>
          <Clip name="soma" cues={[[T.somaPlay - m("soma", "play"), 0], [T.skinSwap - 23, m("soma", "drag")]]} />
        </SlopWindow>
        <SlopWindow x={PET.x} y={PET.y} width={PET.w} height={PET.h} scale={PET.scale} shape="none" at={OPEN.pet} origin={finderIcon(1)} z={30}>
          <Clip name="pet" cues={[[OPEN.pet, 0], [T.petSwap - 14, m("pet", "drag") + 14], [T.petSwap, m("pet", "pet") + 7]]} />
        </SlopWindow>
        <SlopWindow x={CHECK.x} y={CHECK.y} width={CHECK.w} height={CHECK.h} scale={CHECK.scale} at={OPEN.checklist} origin={finderIcon(3)} z={14}>
          <Clip name="checklist" cues={[[OPEN.checklist, m("checklist", "title") + 14, 0], [T.taskAdded - 1, m("checklist", "insert")]]} />
        </SlopWindow>
        <SlopWindow x={TIMER.x} y={TIMER.y} width={TIMER.w} height={TIMER.h} scale={TIMER.scale} shape="ellipse" at={OPEN.timer} origin={finderIcon(2)} z={15}>
          <Clip name="timer" cues={[[OPEN.timer, 0, (m("timer", "theme") - 8) / (T.themeSet - OPEN.timer)], [T.themeSet - 1, m("timer", "theme")]]} />
        </SlopWindow>
        <SlopWindow x={TERM.x} y={TERM.y} width={TERM.w} height={TERM.h} shape="rounded" at={T.termOpen} closeAt={T.termClose} origin={dockPoint(2)} z={60}>
          <Terminal commands={commands} />
        </SlopWindow>

        {montage.map((p, i) => (
          <SlopWindow key={i} x={p.x} y={p.y} width={p.w} height={p.h} scale={p.scale} shape={p.shape ?? "rounded"} at={p.at} origin={{ x: 960, y: 1040 }} z={100 + i}>
            {"clip" in p ? <Clip name={p.clip} loop={p.loop} cues={[[p.at, 0]]} /> : <Img src={staticFile(`captures/still-${p.still}.png`)} style={{ width: "100%", height: "100%" }} />}
          </SlopWindow>
        ))}
      </AbsoluteFill>

      <MenuBar app={app} />
      <div style={{ position: "absolute", inset: 0, transform: `translateY(${(1 - dockRise) * 140}px)`, zIndex: 40 }}>
        <Dock running={running} bounce={frame < T.termOpen + 30 ? { item: frame >= TERM_CLICK ? "terminal" : "finder", at: frame >= TERM_CLICK ? TERM_CLICK : T.finderClick } : undefined} />
      </div>
      {frame < T.outro + 10 && <Cursor at={cursor} clicks={clicks} />}
      <Outro />
    </AbsoluteFill>
  );
}
