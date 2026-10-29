// Engraving and timing with abcjs. Everything here is rebuilt from the stored ABC text;
// nothing from abcjs is ever written to the document.
import abcjs, { type AbcElem, type NoteTimingEvent, type TuneObject } from "abcjs";
import { engraveSource } from "./practice";

/** Timings are taken once at this tempo and scaled to the practice tempo. */
const REFERENCE_BPM = 60;
const SVG = "http://www.w3.org/2000/svg";

export type Note = { pitch: number; whole: number; volume: number };
export type ScoreEvent = {
  /** Milliseconds from the start at the reference tempo (repeats are unrolled). */
  ms: number;
  measure: number;
  line: number;
  left: number;
  endX: number;
  top: number;
  height: number;
  notes: Note[];
  elements: Element[];
  chars: number[];
};
export type Score = {
  tune: TuneObject;
  events: ScoreEvent[];
  /** End of the last pass, in reference milliseconds. */
  endMs: number;
  measures: number;
  beatsPerMeasure: number;
  /** Length of a beat as a fraction of a whole note (1/4, or 3/8 in 6/8). */
  beatLength: number;
  /** Beats before the first downbeat. */
  pickupBeats: number;
  warnings: string[];
};
export type Span = { from: number; to: number };
export type Plan = {
  notes: { t: number; dur: number; pitch: number; volume: number }[];
  clicks: { t: number; accent: boolean }[];
  countIn: { t: number; accent: boolean }[];
  length: number;
  /** Seconds → index into score.events, for the cursor. */
  first: number;
  last: number;
  startMs: number;
  scale: number;
};

// abcjs's click callback has no modifier keys; remember Shift from the pointer itself.
let shiftHeld = false;
if (typeof window !== "undefined")
  window.addEventListener("pointerdown", e => { shiftHeld = e.shiftKey; }, { capture: true });

/** Engraves `abc` into `target` and returns its unrolled timeline. */
export function engrave(target: HTMLElement, abc: string, options: {
  width?: number;
  onPick?: (measure: number, extend: boolean) => void;
} = {}): Score | null {
  let score: Score | null = null;
  const render = abcjs.renderAbc(target, engraveSource(abc), {
    add_classes: true,
    responsive: "resize",
    staffwidth: options.width ?? 680,
    paddingtop: 6,
    paddingbottom: 12,
    paddingleft: 6,
    paddingright: 6,
    selectionColor: "currentColor",
    foregroundColor: "currentColor",
    clickListener: options.onPick ? (elem: AbcElem) => {
      const char = elem.startChar ?? -1;
      const event = score?.events.find(e => e.chars.includes(char));
      if (event) options.onPick!(event.measure, shiftHeld);
    } : undefined,
  });
  const tune = render[0];
  if (!tune || !tune.lines?.length) return null;
  // setUpAudio attaches each note's MIDI pitches, which setTiming then copies into its events.
  try { tune.setUpAudio({}); } catch { /* nothing playable */ }
  tune.setTiming(REFERENCE_BPM);
  const timings = (tune as TuneObject & { noteTimings?: NoteTimingEvent[] }).noteTimings ?? [];
  const events: ScoreEvent[] = [];
  let endMs = 0;
  for (const timing of timings) {
    if (timing.type === "end") { endMs = timing.milliseconds; continue; }
    if (timing.left == null) continue; // the far end of a tie
    events.push({
      ms: timing.milliseconds,
      measure: timing.measureNumber ?? 0,
      line: timing.line ?? 0,
      left: timing.left,
      endX: timing.endX ?? timing.left + (timing.width ?? 10),
      top: timing.top ?? 0,
      height: timing.height ?? 0,
      notes: (timing.midiPitches ?? []).map(p => ({ pitch: p.pitch, whole: p.duration, volume: p.volume })),
      elements: (timing.elements ?? []).flat() as Element[],
      chars: timing.startCharArray ?? (timing.startChar != null ? [timing.startChar] : []),
    });
  }
  if (!events.length) return null;
  const beatLength = tune.getBeatLength() || 0.25;
  score = {
    tune,
    events,
    endMs: endMs || events[events.length - 1].ms + 1000,
    measures: Math.max(...events.map(e => e.measure)) + 1,
    beatsPerMeasure: tune.getBeatsPerMeasure() || 4,
    beatLength,
    pickupBeats: (tune.getPickupLength() || 0) / beatLength,
    warnings: tune.warnings ?? [],
  };
  return score;
}

/** Event indices of the first pass through measures `span.from..span.to`. */
export function spanRange(score: Score, span: Span | null): [number, number] {
  if (!span) return [0, score.events.length];
  const { events } = score;
  let start = events.findIndex(e => e.measure >= span.from && e.measure <= span.to);
  if (start < 0) start = 0;
  let end = start + 1;
  while (end < events.length && events[end].measure >= span.from && events[end].measure <= span.to
    && events[end].measure >= events[end - 1].measure) end++;
  return [start, end];
}

/** The sounds and clicks for one pass at `bpm`, in seconds from the pass start. */
export function planPass(score: Score, span: Span | null, bpm: number): Plan {
  const [first, end] = spanRange(score, span);
  const scale = REFERENCE_BPM / bpm; // reference ms → real ms
  const startMs = score.events[first].ms;
  const endMs = end < score.events.length ? score.events[end].ms : score.endMs;
  const length = (endMs - startMs) * scale / 1000;
  const secondsPerWhole = 60 / bpm / score.beatLength;
  const notes: Plan["notes"] = [];
  for (let i = first; i < end; i++) {
    const event = score.events[i];
    const t = (event.ms - startMs) * scale / 1000;
    for (const note of event.notes)
      notes.push({ t, dur: Math.max(0.05, Math.min(note.whole * secondsPerWhole, length - t + 0.05)), pitch: note.pitch, volume: note.volume });
  }
  const beat = 60 / bpm;
  const offset = first === 0 ? score.pickupBeats % score.beatsPerMeasure : 0;
  const accent = (beats: number) => Math.abs(((beats - offset) % score.beatsPerMeasure + score.beatsPerMeasure) % score.beatsPerMeasure) < 1e-6;
  const clicks: Plan["clicks"] = [];
  const startBeat = offset - Math.floor(offset);
  for (let b = startBeat; b * beat < length - 1e-6; b += 1) clicks.push({ t: b * beat, accent: accent(b) });
  const countIn: Plan["clicks"] = [];
  for (let i = score.beatsPerMeasure; i >= 1; i--) {
    // The bar before the first downbeat; beats under a pickup are already regular clicks.
    const b = offset - i;
    if (b < 0) countIn.push({ t: b * beat, accent: i === score.beatsPerMeasure });
  }
  return { notes, clicks, countIn, length, first, last: end - 1, startMs, scale };
}

/** Index of the event sounding `seconds` into the pass. */
export function eventAt(score: Score, plan: Plan, seconds: number): number {
  const ms = plan.startMs + seconds * 1000 / plan.scale;
  let index = plan.first;
  while (index < plan.last && score.events[index + 1].ms <= ms) index++;
  return index;
}

// ---------------------------------------------------------------- overlay drawn into the SVG

export class Overlay {
  private svg: SVGSVGElement;
  private back: SVGGElement;
  private cursor: SVGRectElement;
  private lit: Element[] = [];

  constructor(target: HTMLElement) {
    this.svg = target.querySelector("svg")!;
    this.back = document.createElementNS(SVG, "g");
    this.back.setAttribute("class", "ps-loop");
    this.svg.insertBefore(this.back, this.svg.firstChild);
    this.cursor = document.createElementNS(SVG, "rect");
    this.cursor.setAttribute("class", "ps-cursor");
    this.cursor.setAttribute("rx", "2");
    this.cursor.setAttribute("width", "3");
    this.cursor.style.display = "none";
    this.svg.appendChild(this.cursor);
  }

  get cursorElement(): Element { return this.cursor; }

  /** Shades the measures of `span` (first pass), one band per engraved line. */
  showSpan(score: Score, span: Span | null) {
    this.back.replaceChildren();
    if (!span) return;
    const [first, end] = spanRange(score, span);
    const lines = new Map<number, { left: number; right: number; top: number; height: number }>();
    for (let i = first; i < end; i++) {
      const e = score.events[i];
      const band = lines.get(e.line);
      if (!band) lines.set(e.line, { left: e.left, right: e.endX, top: e.top, height: e.height });
      else { band.left = Math.min(band.left, e.left); band.right = Math.max(band.right, e.endX); }
    }
    for (const band of lines.values()) {
      const rect = document.createElementNS(SVG, "rect");
      rect.setAttribute("x", String(band.left - 6));
      rect.setAttribute("y", String(band.top - 6));
      rect.setAttribute("width", String(band.right - band.left + 4));
      rect.setAttribute("height", String(band.height + 12));
      rect.setAttribute("rx", "6");
      this.back.appendChild(rect);
    }
  }

  /** Puts the cursor at event `index`, `fraction` of the way to its end. */
  showCursor(score: Score, index: number, fraction: number) {
    const e = score.events[index];
    if (!e) return this.hideCursor();
    const x = e.left + (e.endX - e.left) * Math.min(1, Math.max(0, fraction));
    this.cursor.style.display = "";
    this.cursor.setAttribute("x", String(x - 1.5));
    this.cursor.setAttribute("y", String(e.top - 4));
    this.cursor.setAttribute("height", String(e.height + 8));
    if (this.lit[0] !== e.elements[0]) {
      for (const el of this.lit) el.classList.remove("ps-playing");
      this.lit = e.elements;
      for (const el of this.lit) el.classList.add("ps-playing");
    }
  }

  hideCursor() {
    this.cursor.style.display = "none";
    for (const el of this.lit) el.classList.remove("ps-playing");
    this.lit = [];
  }
}
