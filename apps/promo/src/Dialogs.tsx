import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { sans } from "./Desktop";

const hand = `"Kalam", cursive`;

type Alert = { title: string; body: string; primary: string; secondary: string; label: string; x: number; y: number; rot: number };

const alerts: Alert[] = [
  { title: "Sign in to continue", body: "Create a free account to keep using this app.", primary: "Sign up", secondary: "Log in", label: "no account.", x: 150, y: 120, rot: -3 },
  { title: "Update required", body: "A new version is available. You must update to keep going.", primary: "Update now", secondary: "Later", label: "no forced updates.", x: 1000, y: 150, rot: 2.5 },
  { title: "Your trial has ended", body: "Subscribe for $9.99/month to unlock your documents.", primary: "Subscribe", secondary: "Not now", label: "no subscription.", x: 180, y: 560, rot: 2 },
  { title: "No internet connection", body: "This app needs a connection to open.", primary: "Retry", secondary: "Cancel", label: "works offline.", x: 1020, y: 590, rot: -2.5 },
];

const W = 640;

function AlertCard({ a, enter, strike, clear }: { a: Alert; enter: number; strike: number; clear: number }) {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const pop = spring({ frame: frame - enter, fps, config: { damping: 11, mass: 0.7 } });
  const struck = frame >= strike;
  const draw = interpolate(frame, [strike, strike + 8], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  const label = spring({ frame: frame - strike - 4, fps, config: { damping: 10, mass: 0.6 } });
  const out = interpolate(frame, [clear, clear + 14], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  if (frame < enter || out >= 1) return null;
  const sprawl = out * out;
  return (
    <div style={{ position: "absolute", left: a.x, top: a.y, width: W, opacity: 1 - out, transform: `translateY(${-sprawl * 60}px) rotate(${a.rot + sprawl * (a.rot > 0 ? 14 : -14)}deg) scale(${pop * (1 - out * 0.25)})` }}>
      <div style={{ borderRadius: 26, background: "rgba(255,255,255,.94)", boxShadow: "0 28px 70px rgba(25,12,70,.42), 0 0 0 1px rgba(0,0,0,.16)", padding: "30px 34px 28px", opacity: struck ? 0.55 : 1, fontFamily: sans }}>
        <div style={{ display: "flex", alignItems: "center", gap: 18 }}>
          <div style={{ width: 54, height: 54, borderRadius: 16, background: "#ffd84a", display: "grid", placeItems: "center", fontSize: 34, fontWeight: 800, color: "#10132c" }}>!</div>
          <div style={{ fontSize: 36, fontWeight: 700, color: "#10132c" }}>{a.title}</div>
        </div>
        <div style={{ fontSize: 28, color: "#4e527a", marginTop: 16, lineHeight: 1.3 }}>{a.body}</div>
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 14, marginTop: 24 }}>
          <div style={{ padding: "10px 24px", borderRadius: 14, background: "#ece8f7", fontSize: 26, fontWeight: 600, color: "#3c3960" }}>{a.secondary}</div>
          <div style={{ padding: "10px 24px", borderRadius: 14, background: "#6c16ed", fontSize: 26, fontWeight: 600, color: "#fff" }}>{a.primary}</div>
        </div>
      </div>
      {struck && (
        <>
          <svg width={W} height={260} viewBox={`0 0 ${W} 260`} style={{ position: "absolute", left: 0, top: 0, overflow: "visible", pointerEvents: "none" }}>
            <path d={`M 24 40 C 200 10, 420 30, ${W - 20} 54 M 30 214 C 220 240, 430 220, ${W - 26} 190 M 28 36 L ${W - 24} 224`} stroke="#f443a1" strokeWidth={9} strokeLinecap="round" fill="none" pathLength={1} strokeDasharray={1} strokeDashoffset={1 - draw} />
          </svg>
          <div style={{ position: "absolute", right: -26, bottom: -58, fontFamily: hand, fontSize: 76, fontWeight: 700, color: "#fff", background: "#f443a1", padding: "2px 30px 8px", borderRadius: 18, transform: `rotate(${-a.rot * 1.4 - 3}deg) scale(${label})`, boxShadow: "0 12px 30px rgba(120,10,70,.4)", whiteSpace: "nowrap" }}>{a.label}</div>
        </>
      )}
    </div>
  );
}

/** Parody alerts pile up on the beat, each struck out with what hitSlop does instead; local frame 0 = start of the beat. */
export function NoBeat({ beats, clearAt }: { beats: number[]; clearAt: number }) {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const veil = interpolate(frame, [0, 14], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  const final = spring({ frame: frame - clearAt - 6, fps, config: { damping: 11, mass: 0.8 } });
  const underline = interpolate(frame, [clearAt + 22, clearAt + 40], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  return (
    <AbsoluteFill>
      <AbsoluteFill style={{ background: `rgba(38,22,96,${0.5 * veil})`, backdropFilter: `blur(${veil * 8}px)` }} />
      {alerts.map((a, i) => (
        <AlertCard key={i} a={a} enter={beats[i]} strike={beats[i] + 24} clear={clearAt} />
      ))}
      {frame >= clearAt + 6 && (
        <AbsoluteFill style={{ alignItems: "center", justifyContent: "center" }}>
          <div style={{ fontFamily: hand, fontSize: 190, fontWeight: 700, color: "#fff", transform: `scale(${0.7 + 0.3 * final}) rotate(-3deg)`, opacity: final, textShadow: "0 10px 40px rgba(20,8,70,.5)", position: "relative" }}>
            it's just a file.
            <svg width="100%" height={40} viewBox="0 0 800 40" preserveAspectRatio="none" style={{ position: "absolute", left: 0, bottom: -26 }}>
              <path d="M 10 22 C 180 6, 360 36, 560 14 S 760 24, 792 12" stroke="#f443a1" strokeWidth={12} strokeLinecap="round" fill="none" pathLength={1} strokeDasharray={1} strokeDashoffset={1 - underline} />
            </svg>
          </div>
        </AbsoluteFill>
      )}
    </AbsoluteFill>
  );
}
