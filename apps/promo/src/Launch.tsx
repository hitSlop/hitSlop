import type { ReactNode } from "react";
import {
  AbsoluteFill,
  Audio,
  Img,
  interpolate,
  OffthreadVideo,
  Sequence,
  spring,
  staticFile,
  useCurrentFrame,
  useVideoConfig,
} from "remotion";
import { Promo } from "./Promo";
import { fontFaces, sans, Wallpaper } from "./Desktop";
import { MUSIC } from "./timeline";
import media from "./launch-media.json";

const B = {
  pocket: 135,
  cards: 208,
  timer: 269,
  together: 330,
  agent: 525,
  cascade: 598,
  end: 805,
};
const clamp = { extrapolateLeft: "clamp", extrapolateRight: "clamp" } as const;
type Slug = keyof typeof media;
function Capture({ slug, from = 0, hold = false }: { slug: Slug; from?: number; hold?: boolean }) {
  const f = useCurrentFrame(),
    m = media[slug];
  if (slug === "doodle-board")
    return (
      <Img
        src={staticFile("captures/launch-doodle-native.png")}
        style={{ width: "100%", height: "100%", display: "block" }}
      />
    );
  const frame =
    slug === "pocket-sheet"
      ? m.frames - 1
      : hold
        ? 0
        : Math.min(
            m.frames - 1,
            Math.max(0, f - from) + (slug === "flashcards" ? m.startFrame : 0),
          );
  return (
    <Img
      src={staticFile(`captures/launch-${slug}/${String(frame).padStart(4, "0")}.png`)}
      style={{ width: "100%", height: "100%", display: "block" }}
    />
  );
}
function ObjectWindow({
  children,
  x,
  y,
  w,
  h,
  at = 0,
  tilt = 0,
  z = 1,
}: {
  children: ReactNode;
  x: number;
  y: number;
  w: number;
  h: number;
  at?: number;
  tilt?: number;
  z?: number;
}) {
  const f = useCurrentFrame(),
    { fps } = useVideoConfig();
  if (f < at) return null;
  const s = spring({ frame: f - at, fps, config: { damping: 15, stiffness: 180, mass: 0.7 } });
  return (
    <div
      style={{
        position: "absolute",
        left: x,
        top: y,
        width: w,
        height: h,
        zIndex: z,
        transform: `translateY(${(1 - s) * 90}px) scale(${0.8 + 0.2 * s}) rotate(${tilt}deg)`,
        opacity: Math.min(1, s * 2),
        filter: "drop-shadow(0 20px 25px #20104c35)",
      }}
    >
      {children}
    </div>
  );
}
function Phone({ children, width = 340 }: { children: ReactNode; width?: number }) {
  return (
    <div
      style={{
        width,
        height: width * 2.165,
        padding: 9,
        borderRadius: width * 0.12,
        background: "#191720",
        boxShadow: "0 0 0 2px #ffffff55, 0 25px 45px #19112830",
        overflow: "hidden",
      }}
    >
      <div
        style={{
          width: "100%",
          height: "100%",
          borderRadius: width * 0.095,
          overflow: "hidden",
          background: "#fff9ea",
        }}
      >
        {children}
      </div>
    </div>
  );
}
function Presence({ vertical = false }: { vertical?: boolean }) {
  const f = useCurrentFrame();
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 20,
        color: "#262039",
        fontSize: vertical ? 27 : 25,
        fontWeight: 650,
      }}
    >
      <div style={{ display: "flex", paddingLeft: 10 }}>
        {[
          { initial: "T", color: "#ffd9c1" },
          { initial: "J", color: "#ded3ff" },
        ].map((p, i) => (
          <div
            key={p.initial}
            style={{
              width: 58,
              height: 58,
              borderRadius: 40,
              border: "4px solid #fffaf0",
              background: p.color,
              display: "grid",
              placeItems: "center",
              marginLeft: -10,
              transform: `translateY(${Math.sin((f - i * 5) / 16) * 2}px)`,
            }}
          >
            {p.initial}
          </div>
        ))}
      </div>
      <span>
        <span
          style={{
            display: "inline-block",
            width: 10,
            height: 10,
            borderRadius: 10,
            background: "#458f64",
            marginRight: 10,
          }}
        />
        Editing together
      </span>
    </div>
  );
}
function Together({ vertical }: { vertical: boolean }) {
  return (
    <AbsoluteFill style={{ background: "#fff8e9", fontFamily: sans }}>
      <div style={{ position: "absolute", left: vertical ? 70 : 115, top: vertical ? 110 : 60 }}>
        <div style={{ fontSize: vertical ? 75 : 68, fontWeight: 750, letterSpacing: -3 }}>
          A little day.
          <br />
          Shared.
        </div>
        <div style={{ marginTop: 20 }}>
          <Presence vertical={vertical} />
        </div>
      </div>
      <ObjectWindow
        x={vertical ? 80 : 100}
        y={vertical ? 440 : 345}
        w={vertical ? 920 : 1000}
        h={vertical ? 654 : 711}
        at={0}
        tilt={-2}
      >
        <div style={{ borderRadius: 22, overflow: "hidden" }}>
          <OffthreadVideo
            src={staticFile("footage/little-days-mac.mp4")}
            muted
            style={{ width: "100%", display: "block" }}
          />
        </div>
      </ObjectWindow>
      <ObjectWindow
        x={vertical ? 460 : 1350}
        y={vertical ? 885 : 80}
        w={vertical ? 420 : 400}
        h={vertical ? 910 : 866}
        at={7}
        tilt={3}
        z={3}
      >
        <Phone width={vertical ? 420 : 400}>
          <OffthreadVideo
            src={staticFile("footage/little-days-phone.mp4")}
            muted
            style={{ width: "100%", height: "100%", objectFit: "cover" }}
          />
        </Phone>
      </ObjectWindow>
    </AbsoluteFill>
  );
}
function Agent({ vertical }: { vertical: boolean }) {
  const f = useCurrentFrame();
  const prompt = "Make me a tomato timer. Open a few fun pops, too.";
  const typed = prompt.slice(0, Math.floor(interpolate(f, [0, 37], [0, prompt.length], clamp)));
  return (
    <AbsoluteFill
      style={{
        justifyContent: "center",
        alignItems: "center",
        fontFamily: sans,
        background: "#eee7ff",
      }}
    >
      <div
        style={{
          width: vertical ? 900 : 1150,
          background: "#211e2b",
          borderRadius: 28,
          padding: vertical ? 65 : 55,
          boxShadow: "0 30px 80px #39265333",
          color: "#fffaf4",
        }}
      >
        <div style={{ fontSize: 22, color: "#b4a9c9", marginBottom: 40 }}>you + your agent</div>
        <div
          style={{
            fontSize: vertical ? 55 : 43,
            lineHeight: 1.35,
            minHeight: vertical ? 260 : 130,
          }}
        >
          <span style={{ color: "#ff91cd" }}>› </span>
          {typed}
          <span style={{ opacity: Math.floor(f / 9) % 2 }}>▍</span>
        </div>
        {f > 40 && <div style={{ fontSize: 29, color: "#b6e4b8", marginTop: 30 }}>✦ On it.</div>}
      </div>
    </AbsoluteFill>
  );
}
const cascade: Array<{ slug: Slug; x: number; y: number; w: number; h: number; tilt: number }> = [
  { slug: "pocket-pod", x: 1090, y: 70, w: 310, h: 519, tilt: 3 },
  { slug: "flashcards", x: 70, y: 90, w: 600, h: 489, tilt: -3 },
  { slug: "focus-timer", x: 670, y: 50, w: 350, h: 350, tilt: 8 },
  { slug: "baby-journal", x: 360, y: 430, w: 780, h: 555, tilt: 2 },
  { slug: "koi-pond", x: 1410, y: 400, w: 430, h: 430, tilt: 0 },
  { slug: "doodle-board", x: 700, y: 240, w: 530, h: 423, tilt: -5 },
  { slug: "wordle", x: 80, y: 500, w: 360, h: 507, tilt: -6 },
  { slug: "pocket-sheet", x: 1240, y: 210, w: 500, h: 420, tilt: 5 },
  { slug: "pixel-art", x: 1140, y: 490, w: 330, h: 445, tilt: -3 },
  { slug: "codex-pet", x: 1510, y: 750, w: 280, h: 210, tilt: 0 },
];
function Cascade({ vertical }: { vertical: boolean }) {
  const f = useCurrentFrame();
  return (
    <AbsoluteFill>
      <Wallpaper />
      {cascade.map((p, i) => {
        const x = vertical ? [30, 190, 630, 30, 610, 220, 40, 340, 610, 690][i] : p.x;
        const y = vertical ? [150, 190, 70, 700, 1100, 520, 1150, 1040, 1250, 1570][i] : p.y;
        const w = vertical ? p.w * 0.83 : p.w;
        const at = [0, 24, 48, 73, 85, 97, 109, 121, 133, 146][i];
        return (
          <ObjectWindow
            key={p.slug}
            x={x}
            y={y}
            w={w}
            h={p.h * (w / p.w)}
            at={at}
            tilt={p.tilt}
            z={i + 1}
          >
            <>
              {p.slug === "baby-journal" ? (
                <Img
                  src={staticFile("captures/launch-days-native.png")}
                  style={{ width: "100%", height: "100%", display: "block" }}
                />
              ) : (
                <Capture slug={p.slug} from={at} />
              )}
            </>
          </ObjectWindow>
        );
      })}
    </AbsoluteFill>
  );
}
function End({ vertical }: { vertical: boolean }) {
  const f = useCurrentFrame(),
    { fps } = useVideoConfig();
  const s = spring({ frame: f, fps, config: { damping: 16 } });
  return (
    <AbsoluteFill
      style={{
        background: "#fff9ee",
        alignItems: "center",
        justifyContent: "center",
        fontFamily: sans,
        color: "#141225",
      }}
    >
      <Img
        src={staticFile("brand/wordmark.webp")}
        style={{
          width: vertical ? 600 : 430,
          transform: `scale(${0.9 + 0.1 * s})`,
          opacity: s,
          marginBottom: vertical ? 100 : 60,
        }}
      />
      <div
        style={{
          fontSize: vertical ? 100 : 95,
          fontWeight: 750,
          lineHeight: 1.06,
          letterSpacing: -4,
          textAlign: "center",
        }}
      >
        Little apps.
        <br />
        <span style={{ color: "#e5197f" }}>Better together.</span>
      </div>
      <div style={{ fontSize: vertical ? 39 : 34, marginTop: vertical ? 100 : 50 }}>
        dotpop.click
      </div>
      <div style={{ fontSize: vertical ? 24 : 22, color: "#81768a", marginTop: 22 }}>
        Mac + iPhone · iPhone in TestFlight
      </div>
    </AbsoluteFill>
  );
}
function Hero({
  slug,
  vertical,
  phone = false,
}: {
  slug: Slug;
  vertical: boolean;
  phone?: boolean;
}) {
  const m = media[slug];
  const w = vertical ? 720 : slug === "focus-timer" ? 650 : slug === "flashcards" ? 940 : 490;
  const h = (w * m.height) / m.width;
  return (
    <AbsoluteFill>
      <Wallpaper />
      <div
        style={{
          position: "absolute",
          left: vertical ? 70 : 110,
          top: vertical ? 135 : 95,
          color: "#211936",
          fontFamily: sans,
          fontSize: vertical ? 65 : 64,
          fontWeight: 750,
          letterSpacing: -2,
        }}
      >
        {slug === "pocket-pod"
          ? "Your pocket soundtrack."
          : slug === "flashcards"
            ? "A little more curious."
            : "Make a little time."}
      </div>
      <ObjectWindow
        x={vertical ? (1080 - w) / 2 : phone ? 460 : (1920 - w) / 2}
        y={vertical ? 420 : 210}
        w={w}
        h={h}
        at={0}
        tilt={-2}
      >
        <Capture slug={slug} />
      </ObjectWindow>
      {phone && (
        <ObjectWindow
          x={vertical ? 610 : 1200}
          y={vertical ? 1010 : 160}
          w={vertical ? 350 : 360}
          h={780}
          at={12}
          tilt={4}
          z={3}
        >
          <Phone width={vertical ? 350 : 360}>
            <Capture slug="pocket-pod-phone" />
          </Phone>
        </ObjectWindow>
      )}
    </AbsoluteFill>
  );
}
export function Launch({ vertical = false }: { vertical?: boolean }) {
  const frame = useCurrentFrame();
  return (
    <AbsoluteFill style={{ overflow: "hidden", background: "#fff9ee" }}>
      <style>{fontFaces}</style>
      <Sequence from={134} layout="none">
        <Audio
          src={staticFile("music/new-york.mp3")}
          startFrom={Math.round(MUSIC.songStart * 30)}
          volume={(f) => interpolate(f + 134, [870, 899], [1, 0], clamp)}
        />
      </Sequence>
      <Sequence durationInFrames={B.pocket}>
        <Wallpaper />
        <div
          style={{
            position: "absolute",
            width: 1920,
            height: 1080,
            transformOrigin: "top left",
            transform: vertical
              ? `translate(${interpolate(frame, [85, 120], [-20, -650], clamp)}px, 430px)`
              : "none",
          }}
        >
          <Promo audio={false} />
        </div>
      </Sequence>
      <Sequence from={B.pocket} durationInFrames={B.cards - B.pocket}>
        <Hero slug="pocket-pod" vertical={vertical} phone />
      </Sequence>
      <Sequence from={B.cards} durationInFrames={B.timer - B.cards}>
        <Hero slug="flashcards" vertical={vertical} />
      </Sequence>
      <Sequence from={B.timer} durationInFrames={B.together - B.timer}>
        <Hero slug="focus-timer" vertical={vertical} />
      </Sequence>
      <Sequence from={B.together} durationInFrames={B.agent - B.together}>
        <Together vertical={vertical} />
      </Sequence>
      <Sequence from={B.agent} durationInFrames={B.cascade - B.agent}>
        <Agent vertical={vertical} />
      </Sequence>
      <Sequence from={B.cascade} durationInFrames={B.end - B.cascade}>
        <Cascade vertical={vertical} />
      </Sequence>
      <Sequence from={B.end}>
        <End vertical={vertical} />
      </Sequence>
    </AbsoluteFill>
  );
}
export const LaunchWide = () => <Launch />;
export const LaunchVertical = () => <Launch vertical />;
