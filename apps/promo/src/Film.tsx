import { AbsoluteFill, Audio, Freeze, interpolate, OffthreadVideo, Sequence, staticFile, useCurrentFrame } from "remotion";
import { NoBeat } from "./Dialogs";
import { Promo } from "./Promo";
import { MUSIC, T } from "./timeline";

/**
 * The film is the 30 s desktop tour (Promo) played continuously from frame PRE, with footage of the girl
 * cut over it on UI actions. Because Promo never stops, its beat grid (music.json) stays valid:
 * film frame = promo frame + PRE. Footage is video only; the song is mixed here.
 */
const PRE = 150;
const at = (promoFrame: number) => promoFrame + PRE;

const NO_FROM = at(800); // after the montage; Promo is frozen on its last full desktop behind the alerts
const NO_LEN = 320;
const STRETCH_FROM = NO_FROM + NO_LEN;
const STRETCH_LEN = 60;
const CLOSE_FROM = STRETCH_FROM + STRETCH_LEN;
const CLOSE_LEN = 100;
const END_FROM = CLOSE_FROM + CLOSE_LEN;
const END_LEN = 100;
export const FILM_DURATION = END_FROM + END_LEN;

// Promo beats: first at 135.3, then every 12.167 frames (148 BPM). Extended past the end of music.json.
const BEAT = 1800 / MUSIC.bpm;
const beatAt = (k: number) => MUSIC.beats[0] + k * BEAT;
const noBeats = [0, 4, 8, 12].map((i) => beatAt(56 + i) - 800);
const noClear = beatAt(72) - 800;

type Cut = { file: string; from: number; len: number; start?: number; push?: number };

function Footage({ file, start = 0, len, push = 0.05 }: Omit<Cut, "from">) {
  const frame = useCurrentFrame();
  const scale = 1 + push * (frame / len);
  return (
    // Promo's dock and menu bar carry high z-indexes in the shared stacking context; footage must sit above them.
    <AbsoluteFill style={{ background: "#000", zIndex: 5000 }}>
      <OffthreadVideo src={staticFile(`footage/${file}.mp4`)} startFrom={start} muted style={{ width: "100%", height: "100%", objectFit: "cover", transform: `scale(${scale})` }} />
      <AbsoluteFill style={{ background: "radial-gradient(ellipse at 50% 50%, rgba(0,0,0,0) 55%, rgba(20,8,50,.45) 100%)" }} />
    </AbsoluteFill>
  );
}

const cuts: Cut[] = [
  // Preface: she opens the laptop; the glow lands on her face, then a small smile.
  { file: "s1-lid", from: 0, len: 100 },
  { file: "s2-smile", from: 100, len: 50, start: 80, push: 0.02 },
  // Finger double-clicks the trackpad, and the app opens on the cut.
  { file: "s3-trackpad", from: at(58), len: 35, start: 30 },
  // Headphones go on as the song plays.
  { file: "s4-headphones", from: at(138), len: 32 },
  // She laughs at the re-skinned player.
  { file: "s5-laugh", from: at(212), len: 48 },
  // Chin in hand, pastel light on her cheek, while the CLI does its work.
  { file: "s6-ots", from: at(500), len: 48, start: 30 },
  // Relief, then the lid closes.
  { file: "s7-stretch", from: STRETCH_FROM, len: STRETCH_LEN, start: 20, push: 0.03 },
  { file: "s8-close", from: CLOSE_FROM, len: CLOSE_LEN, push: 0.03 },
];

export function Film() {
  return (
    <AbsoluteFill style={{ background: "#000" }}>
      {/* The song kicks in on SomaAmp's play press, as in Promo, and fades out over the last 1.5 s. */}
      <Sequence from={at(T.somaPlay)} layout="none">
        <Audio
          src={staticFile("music/new-york.mp3")}
          startFrom={Math.round(MUSIC.songStart * 30)}
          volume={(f) => interpolate(f + at(T.somaPlay), [FILM_DURATION - 45, FILM_DURATION - 1], [1, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp" })}
        />
      </Sequence>

      <Sequence from={PRE} durationInFrames={800}>
        <Promo audio={false} />
      </Sequence>

      <Sequence from={NO_FROM} durationInFrames={NO_LEN}>
        <Freeze frame={790}>
          <Promo audio={false} />
        </Freeze>
        <NoBeat beats={noBeats} clearAt={noClear} />
      </Sequence>

      {/* End card: Promo's own outro, over the montage desktop. */}
      <Sequence from={END_FROM} durationInFrames={END_LEN}>
        <Sequence from={-800}>
          <Promo audio={false} />
        </Sequence>
      </Sequence>

      {cuts.map((c) => (
        <Sequence key={c.file} from={c.from} durationInFrames={c.len}>
          <Footage file={c.file} start={c.start} len={c.len} push={c.push} />
        </Sequence>
      ))}
    </AbsoluteFill>
  );
}
