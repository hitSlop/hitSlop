import { capture } from "@hitslop/document/capture";
import p5 from "./p5-global";
import * as brush from "p5.brush";
import type { KoiPattern } from "./schema";

// Everything is authored in a 600-unit square; the canvases scale it to the window.
const SIZE = 600;
const C = SIZE / 2;
const WATER = 262;
const SWIM = 200;

export type Palette = Record<
  "water" | "deep" | "shallow" | "stone" | "moss" | "lily" | "lotus" | "paper" | "ink" | "cream" | "red" | "gold" | "charcoal" | "slate" | "orange",
  string
>;
export type KoiSpec = { id: string; name: string; pattern: KoiPattern; size: number };
type Spot = { u: number; v: number; r: number; color: string };
type Pellet = { x: number; y: number; born: number };
type Ripple = { x: number; y: number; born: number; strength: number };

const mulberry = (seed: number) => () => {
  seed = (seed + 0x6d2b79f5) | 0;
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
};
const hash = (text: string) => {
  let h = 2166136261;
  for (let i = 0; i < text.length; i++) h = Math.imul(h ^ text.charCodeAt(i), 16777619);
  return h >>> 0;
};
const wrap = (a: number) => Math.atan2(Math.sin(a), Math.cos(a));
const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

function mix(a: string, b: string, t: number) {
  const pa = parseInt(a.slice(1), 16), pb = parseInt(b.slice(1), 16);
  const ch = (shift: number) => Math.round(((pa >> shift) & 255) * (1 - t) + ((pb >> shift) & 255) * t);
  return `#${((1 << 24) | (ch(16) << 16) | (ch(8) << 8) | ch(0)).toString(16).slice(1)}`;
}
function alpha(hex: string, a: number) {
  const p = parseInt(hex.slice(1), 16);
  return `rgba(${(p >> 16) & 255},${(p >> 8) & 255},${p & 255},${a})`;
}

function koiColors(pattern: KoiPattern, palette: Palette, rand: () => number) {
  const spots: Spot[] = [];
  const spot = (u: number, v: number, r: number, color: string) => spots.push({ u, v, r, color });
  switch (pattern) {
    case "kohaku":
      for (let i = 0, n = 2 + Math.floor(rand() * 2); i < n; i++) spot(0.12 + i * 0.26 + rand() * 0.08, (rand() - 0.5) * 0.9, 0.75 + rand() * 0.4, palette.red);
      return { base: palette.cream, fin: palette.cream, spots };
    case "showa":
      for (let i = 0; i < 3; i++) spot(0.1 + i * 0.25 + rand() * 0.1, (rand() - 0.5) * 1.2, 0.7 + rand() * 0.3, palette.red);
      for (let i = 0; i < 2; i++) spot(0.3 + rand() * 0.45, (rand() > 0.5 ? 1 : -1) * (0.5 + rand() * 0.5), 0.45 + rand() * 0.3, palette.cream);
      return { base: palette.charcoal, fin: mix(palette.charcoal, palette.cream, 0.35), spots };
    case "ogon":
      for (let i = 0; i < 5; i++) spot(0.12 + i * 0.13, 0, 0.35, mix(palette.gold, "#ffffff", 0.45));
      return { base: palette.gold, fin: mix(palette.gold, "#ffffff", 0.3), spots };
    case "asagi":
      for (let i = 0; i < 3; i++) { spot(0.25 + i * 0.2, 1.05, 0.55, palette.orange); spot(0.25 + i * 0.2, -1.05, 0.55, palette.orange); }
      spot(0.06, 0, 0.55, mix(palette.slate, "#ffffff", 0.55));
      return { base: palette.slate, fin: mix(palette.orange, palette.cream, 0.35), spots };
    case "tancho":
      spot(0.1, 0, 0.62, palette.red);
      return { base: palette.cream, fin: palette.cream, spots };
  }
}

class Fish {
  spine: { x: number; y: number }[] = [];
  heading: number;
  speed = 16;
  phase: number;
  noise: number;
  fade = 0;
  leaving = false;
  gulp = 0;
  length: number;
  segment: number;
  colors: ReturnType<typeof koiColors>;

  constructor(public spec: KoiSpec, palette: Palette, x: number, y: number, heading: number) {
    const rand = mulberry(hash(spec.id));
    this.heading = heading;
    this.phase = rand() * 10;
    this.noise = rand() * 1000;
    this.length = 88 * spec.size;
    this.segment = this.length / 12;
    this.colors = koiColors(spec.pattern, palette, rand);
    for (let i = 0; i <= 12; i++) this.spine.push({ x: x - Math.cos(heading) * this.segment * i, y: y - Math.sin(heading) * this.segment * i });
  }
  recolor(palette: Palette) { this.colors = koiColors(this.spec.pattern, palette, mulberry(hash(this.spec.id))); }
  get head() { return this.spine[0]!; }
}

export type SceneOptions = {
  host: HTMLElement;
  palette: Palette;
  seed: number;
  reducedMotion: boolean;
  onFeed: (fed: string | undefined) => void;
  /** True while a new seed is being painted; the previous pond stays live meanwhile. */
  onPaintingChange?: (busy: boolean) => void;
};

export class PondScene {
  private painter!: p5;
  private animator!: p5;
  private water = document.createElement("canvas");
  private lilies = document.createElement("canvas");
  private previous = document.createElement("canvas");
  private previousLilies = document.createElement("canvas");
  private job: Generator<void> | undefined;
  private painted = false;
  private fadeStart = -1;
  private paintedSeed = -1;
  private paintedPixels = 0;
  private painting: Promise<void> = Promise.resolve();
  private fish = new Map<string, Fish>();
  private pellets: Pellet[] = [];
  private ripples: Ripple[] = [];
  private nextAmbient = 3;
  private hover: { x: number; y: number } | undefined;
  private last = 0;
  private time = 0;
  private hidden = false;
  private resizeTimer: ReturnType<typeof setTimeout> | undefined;
  private observer: ResizeObserver;
  private started: Promise<void>;
  private disposed = false;
  private pixels = 0;
  private capturePass = false;
  private paintEpoch = 0;
  palette: Palette;
  seed: number;
  reducedMotion: boolean;

  constructor(private options: SceneOptions) {
    this.palette = options.palette;
    this.seed = options.seed;
    this.reducedMotion = options.reducedMotion;
    const hidden = document.createElement("div");
    hidden.className = "koi-painter";
    hidden.setAttribute("aria-hidden", "true");
    options.host.append(hidden);
    this.started = new Promise<void>((resolve) => {
      let painterReady = false, animatorReady = false;
      const done = () => { if (painterReady && animatorReady) resolve(); };
      this.painter = new p5((p: p5) => {
        brush.instance(p);
        p.setup = () => {
          p.createCanvas(SIZE, SIZE, p.WEBGL);
          p.pixelDensity(1);
          p.angleMode(p.DEGREES);
          brush.scaleBrushes(3);
          p.noLoop();
          painterReady = true;
          done();
        };
        p.draw = () => this.paintSlice(p);
      }, hidden);
      this.animator = new p5((p: p5) => {
        p.setup = () => {
          const canvas = p.createCanvas(SIZE, SIZE);
          canvas.elt.setAttribute("aria-hidden", "true");
          p.frameRate(30);
          p.noLoop();
          animatorReady = true;
          done();
        };
        p.draw = () => this.frame(p);
        p.mousePressed = (event?: MouseEvent) => { if (event && event.button === 0 && event.target === p.canvas) this.click(p.mouseX, p.mouseY); };
        p.mouseMoved = () => { this.hover = { x: p.mouseX, y: p.mouseY }; if (!this.looping) void p.redraw(); };
      }, options.host);
    });
    this.observer = new ResizeObserver(() => this.resize());
    this.observer.observe(options.host);
    options.host.addEventListener("pointerleave", this.leave);
    document.addEventListener("visibilitychange", this.visibility);
    void this.started.then(() => { this.resize(true); });
  }

  /** Resolves once the current seed is painted at the current size. */
  async ready() {
    await this.started;
    let pending: Promise<void>;
    do { pending = this.painting; await pending; } while (pending !== this.painting);
  }

  private get looping() { return !this.reducedMotion && !this.hidden && !this.disposed; }

  private leave = () => { this.hover = undefined; if (!this.looping) void this.animator?.redraw(); };
  private visibility = () => { this.hidden = document.hidden; this.updateLoop(); };

  private updateLoop() {
    if (!this.animator || this.disposed) return;
    if (this.looping) { this.last = performance.now(); this.animator.loop(); }
    else { this.animator.noLoop(); void this.animator.redraw(); }
  }

  private resize(immediate = false) {
    if (this.disposed || !this.animator) return;
    const box = this.options.host.getBoundingClientRect();
    const side = Math.max(120, Math.floor(Math.min(box.width, box.height)));
    const density = Math.min(window.devicePixelRatio || 1, 2);
    this.animator.resizeCanvas(side, side);
    this.animator.pixelDensity(density);
    // Watercolor is soft; painting above 1.5× costs time without visible detail.
    // Capture uses a smaller plate so the still frame finishes within the screenshot budget.
    this.pixels = capture.isRenderer()
      ? Math.max(160, Math.round(side * 0.5))
      : Math.round(side * Math.min(density, 1.5));
    // Stretch the existing painting while resizing, and repaint once it settles.
    clearTimeout(this.resizeTimer);
    const repaint = () => { if (Math.abs(this.pixels - this.paintedPixels) > this.paintedPixels * 0.2 || this.paintedSeed !== this.seed) this.repaint(); };
    if (immediate) repaint(); else this.resizeTimer = setTimeout(repaint, 350);
    this.updateLoop();
  }

  setSeed(seed: number) {
    if (seed === this.seed) return;
    this.seed = seed;
    this.repaint();
  }
  setPalette(palette: Palette) {
    this.palette = palette;
    for (const fish of this.fish.values()) fish.recolor(palette);
    this.paintedSeed = -1;
    this.repaint();
  }
  setReducedMotion(reduced: boolean) {
    this.reducedMotion = reduced;
    this.updateLoop();
  }

  setKoi(list: KoiSpec[]) {
    const rand = mulberry(hash(list.map((k) => k.id).join()) ^ Date.now());
    const initial = this.fish.size === 0;
    const ids = new Set(list.map((k) => k.id));
    for (const [id, fish] of this.fish) if (!ids.has(id) && !fish.leaving) { fish.leaving = true; this.ripple(fish.head.x, fish.head.y, 1); }
    for (const spec of list) {
      const existing = this.fish.get(spec.id);
      if (existing) {
        const recolor = existing.spec.pattern !== spec.pattern;
        existing.spec = spec;
        if (recolor) existing.recolor(this.palette);
        continue;
      }
      const angle = rand() * Math.PI * 2, radius = Math.sqrt(rand()) * SWIM * 0.8;
      const fish = new Fish(spec, this.palette, C + Math.cos(angle) * radius, C + Math.sin(angle) * radius, rand() * Math.PI * 2);
      if (initial || this.reducedMotion) fish.fade = 1;
      else this.ripple(fish.head.x, fish.head.y, 1.2);
      this.fish.set(spec.id, fish);
    }
    if (!this.looping) void this.animator?.redraw();
  }

  /** Drops pellets at a point in canvas pixels, or somewhere open when omitted. */
  feed(at?: { x: number; y: number }) {
    const rand = Math.random;
    let x: number, y: number;
    if (at) ({ x, y } = at);
    else { const a = rand() * Math.PI * 2, r = 40 + rand() * 110; x = C + Math.cos(a) * r; y = C + Math.sin(a) * r; }
    if (Math.hypot(x - C, y - C) > WATER - 12) return undefined;
    if (this.reducedMotion) {
      // Without motion, the nearest koi takes the food immediately.
      return this.nearest(x, y)?.spec.name;
    }
    for (let i = 0; i < 4; i++) this.pellets.push({ x: x + (rand() - 0.5) * 22, y: y + (rand() - 0.5) * 22, born: this.time + i * 0.08 });
    this.pellets = this.pellets.slice(-32);
    this.ripple(x, y, 0.8);
    return undefined;
  }

  /** A still frame of the current pond, used by the export view. */
  async snapshot() {
    // Drop any interactive time-sliced plate and paint a smaller one in a single pass.
    // The screenshot budget is 10s, and a full watercolor does not fit in it.
    this.paintEpoch++;
    this.job = undefined;
    this.pixels = Math.min(this.pixels || 220, 220);
    this.paintedSeed = -1;
    this.capturePass = true;
    this.repaint();
    try {
      await this.ready();
      await this.animator.redraw();
      return this.animator.canvas.toDataURL("image/png");
    } finally {
      this.capturePass = false;
    }
  }

  destroy() {
    this.disposed = true;
    clearTimeout(this.resizeTimer);
    this.observer.disconnect();
    this.options.host.removeEventListener("pointerleave", this.leave);
    document.removeEventListener("visibilitychange", this.visibility);
    this.animator?.remove();
    this.painter?.remove();
    this.options.host.querySelector(".koi-painter")?.remove();
  }

  private click(px: number, py: number) {
    const scale = SIZE / this.animator.width;
    const x = px * scale, y = py * scale;
    if (!this.nearest(x, y) || Math.hypot(x - C, y - C) > WATER - 12) return;
    this.options.onFeed(this.feed({ x, y }));
  }

  private nearest(x: number, y: number) {
    let best: Fish | undefined, distance = Infinity;
    for (const fish of this.fish.values()) {
      if (fish.leaving) continue;
      const d = Math.hypot(fish.head.x - x, fish.head.y - y);
      if (d < distance) { distance = d; best = fish; }
    }
    return best;
  }

  private ripple(x: number, y: number, strength: number) {
    if (this.reducedMotion) return;
    this.ripples.push({ x, y, born: this.time, strength });
    if (this.ripples.length > 24) this.ripples.shift();
  }

  // ── Painting (p5.brush, WEBGL, runs only on seed or size changes) ────────────

  private pads: [number, number][][] = [];

  /**
   * Paints the current seed in short slices so the koi keep swimming meanwhile. Requests
   * coalesce: each queued run paints whatever is current, and a newer seed aborts a stale one.
   */
  private repaint() {
    this.painting = this.painting.then(async () => {
      const seed = this.seed, pixels = this.pixels, epoch = this.paintEpoch;
      if (this.disposed || !this.painter || !pixels || epoch !== this.paintEpoch) return;
      if (seed === this.paintedSeed && pixels === this.paintedPixels) return;
      const crossfade = this.painted && this.paintedSeed >= 0 && seed !== this.paintedSeed && !this.reducedMotion;
      if (this.paintedSeed >= 0 && seed !== this.paintedSeed) this.options.onPaintingChange?.(true);
      if (this.painter.width !== pixels) this.painter.resizeCanvas(pixels, pixels);
      const layers: HTMLCanvasElement[] = [];
      for (const layer of ["water", "lilies"] as const) {
        this.painter.randomSeed(seed + (layer === "lilies" ? 7919 : 0));
        this.painter.noiseSeed(seed);
        this.job = layer === "water" ? this.paintWater(this.painter) : this.paintLilies(this.painter);
        while (this.job) {
          if (epoch !== this.paintEpoch) { this.job = undefined; return; }
          await this.painter.redraw();
          if (this.disposed || this.seed !== seed || epoch !== this.paintEpoch) { this.job = undefined; return; }
          // Let the animator draw a frame between slices. Capture paints straight through
          // so the screenshot does not wait out the interactive time-slicing.
          if (!this.capturePass && !this.paintingFast) await new Promise((resolve) => setTimeout(resolve, 0));
        }
        layers.push(this.cutOut(layer, pixels));
      }
      if (crossfade) { this.previous = this.water; this.previousLilies = this.lilies; }
      [this.water, this.lilies] = layers as [HTMLCanvasElement, HTMLCanvasElement];
      this.painted = true;
      this.paintedSeed = seed;
      this.paintedPixels = pixels;
      this.fadeStart = crossfade ? this.time : -1;
      if (!this.looping) void this.animator.redraw();
    }).catch((error) => console.error("Koi Pond could not paint", error))
      .finally(() => { if (!this.disposed) this.options.onPaintingChange?.(this.paintedSeed >= 0 && this.paintedSeed !== this.seed); });
  }

  /** Copies the painter into a layer, cutting the pond or the pads out of the opaque ground. */
  private cutOut(layer: "water" | "lilies", pixels: number) {
    const target = document.createElement("canvas");
    target.width = pixels; target.height = pixels;
    const ctx = target.getContext("2d")!;
    ctx.drawImage(this.painter.canvas, 0, 0, pixels, pixels);
    ctx.globalCompositeOperation = "destination-in";
    ctx.scale(pixels / SIZE, pixels / SIZE);
    ctx.filter = `blur(${layer === "water" ? 1.5 : 1}px)`;
    ctx.beginPath();
    if (layer === "water") ctx.arc(C, C, C - 3, 0, Math.PI * 2);
    else for (const edge of this.pads) { edge.forEach(([x, y], i) => (i ? ctx.lineTo(x, y) : ctx.moveTo(x, y))); ctx.closePath(); }
    ctx.fill();
    return target;
  }

  /** Screenshot capture has no swimming koi to keep responsive, so the plate can finish in one pass. */
  private get paintingFast() {
    return capture.isRenderer() || document.documentElement.hasAttribute("data-slop-capture");
  }

  /** One painter redraw: runs the active paint job for about 8ms. */
  private paintSlice(p: p5) {
    if (!this.job) return;
    brush.load();
    // The matrix resets each redraw. No push/pop: p5.brush ties its fill/stroke state to them.
    p.translate(-p.width / 2, -p.height / 2);
    p.scale(p.width / SIZE);
    if (this.capturePass || this.paintingFast) {
      while (this.job && !this.job.next().done) {}
      this.job = undefined;
      return;
    }
    const until = performance.now() + 8;
    while (this.job && performance.now() < until) {
      if (this.job.next().done) { this.job = undefined; break; }
    }
  }

  private *paintWater(p: p5): Generator<void> {
    const pal = this.palette;
    const r = (lo: number, hi: number) => p.random(lo, hi);
    const ring = (lo: number, hi: number) => { const a = r(0, 360), d = Math.sqrt(r(lo * lo, hi * hi)); return [C + p.cos(a) * d, C + p.sin(a) * d] as const; };
    // p5.brush fills are translucent glazes, so each shape gets a flat wash first.
    const shape = function* (color: string, opacity: number, glazes: number, shapes: (() => void)[]) {
      brush.wash(color, opacity);
      for (const draw of shapes) { draw(); yield; }
      brush.noWash();
      if (!glazes) return;
      brush.fill(mix(color, pal.ink, 0.12), 255);
      for (let i = 0; i < glazes; i++) for (const draw of shapes) { draw(); yield; }
      brush.noFill();
    };
    p.background(pal.paper);
    brush.noStroke();
    // Damp earth under the stones, then an even body of water.
    brush.fillBleed(0.15);
    brush.fillTexture(0.6, 0.5);
    yield* shape(mix(pal.moss, pal.stone, 0.45), 255, 0, [() => brush.circle(C, C, C + 4, 0)]);
    yield* shape(pal.water, 255, 0, [() => brush.circle(C, C, WATER + 4, 0.03)]);
    // Depth: glazes pooling darker toward the middle, lighter near the rim.
    brush.fillTexture(0.7, 0.85);
    brush.fillBleed(0.3, "out");
    brush.fill(mix(pal.deep, pal.water, 0.25), 255);
    const deep = [C + r(-25, 25), C + r(-25, 25)] as const;
    for (let i = 0; i < 3; i++) { brush.circle(deep[0] + r(-12, 12), deep[1] + r(-12, 12), r(120, 175) - i * 25, 0.45); yield; }
    brush.fill(pal.shallow, 255);
    brush.fillBleed(0.25, "in");
    for (let i = 0; i < 6; i++) { brush.circle(...ring(WATER - 60, WATER - 25), r(28, 46), 0.6); yield; }
    brush.noFill();
    // Pebbles resting on the floor.
    brush.wash(mix(pal.stone, pal.water, 0.45), 200);
    for (let i = 0; i < 24; i++) brush.circle(...ring(30, WATER - 30), r(3, 7), 0.4);
    brush.noWash();
    yield;
    // Swaying weeds near the edge.
    brush.field("curved");
    brush.set("cpencil", mix(pal.moss, pal.deep, 0.35), 1.6);
    for (let i = 0; i < 30; i++) {
      const a = r(0, 360), d = WATER - r(6, 30);
      brush.flowLine(C + p.cos(a) * d, C + p.sin(a) * d, r(26, 60), a + 180 + r(-40, 40));
      if (i % 6 === 5) yield;
    }
    brush.noField();
    // Stones around the rim, grouped by tone.
    const tones = [mix(pal.stone, pal.paper, 0.25), mix(pal.stone, pal.paper, 0.55), mix(pal.stone, pal.ink, 0.1)];
    const stones: [number, number, number][][] = [[], [], []];
    for (let a = r(0, 10); a < 360; a += r(12, 17)) {
      const radius = r(22, 31), d = WATER + radius * 0.5;
      stones[Math.floor(r(0, 3))]!.push([C + p.cos(a) * d, C + p.sin(a) * d, radius]);
    }
    brush.noStroke();
    brush.fillBleed(0.05);
    brush.fillTexture(0.6, 0.9);
    for (const [i, group] of stones.entries()) yield* shape(tones[i]!, 240, 1, group.map(([x, y, radius]) => () => brush.circle(x, y, radius, 0.35)));
    brush.set("HB", mix(pal.stone, pal.ink, 0.55), 0.8);
    for (const group of stones) { for (const [x, y, radius] of group) { const a = r(0, 360); brush.arc(x, y, radius * 0.94, a, a + r(170, 300)); } yield; }
    // Moss tucked between stones.
    brush.noStroke();
    brush.fillBleed(0.12);
    brush.fillTexture(0.5, 0.6);
    const moss: [number, number, number][] = [];
    for (let i = 0; i < 12; i++) { const a = r(0, 360), d = WATER + r(2, 18); moss.push([C + p.cos(a) * d, C + p.sin(a) * d, r(7, 13)]); }
    yield* shape(pal.moss, 220, 1, moss.map(([x, y, radius]) => () => brush.circle(x, y, radius, 0.6)));
    // A soft pencil line where the water meets the stones, and a few glints.
    brush.set("2B", mix(pal.deep, pal.ink, 0.5), 0.8);
    brush.circle(C, C, WATER - 2, 0.05);
    yield;
    brush.set("2H", mix(pal.shallow, "#ffffff", 0.5), 1.3);
    for (let i = 0; i < 6; i++) {
      const a = r(0, 360);
      brush.arc(C, C, WATER - r(18, 50), a, a + r(15, 40));
    }
    brush.noStroke();
  }

  private *paintLilies(p: p5): Generator<void> {
    const pal = this.palette;
    p.background(mix(pal.water, pal.deep, 0.25));
    const r = (lo: number, hi: number) => p.random(lo, hi);
    type Pad = { x: number; y: number; radius: number; notch: number; edge: [number, number][] };
    const pads: Pad[] = [];
    for (let tries = 0; pads.length < 5 && tries < 200; tries++) {
      const a = r(0, 360), d = r(60, WATER - 60), radius = r(24, 42);
      const x = C + p.cos(a) * d, y = C + p.sin(a) * d;
      if (Math.hypot(x - C, y - C) + radius > WATER - 16) continue;
      if (pads.some((pad) => Math.hypot(pad.x - x, pad.y - y) < pad.radius + radius + 16)) continue;
      const notch = r(0, 360);
      const edge: [number, number][] = [[x + p.cos(notch) * radius * 0.12, y + p.sin(notch) * radius * 0.12]];
      for (let t = 18; t <= 342; t += 9) {
        const reach = radius * (1 + r(-0.025, 0.025));
        edge.push([x + p.cos(notch + t) * reach, y + p.sin(notch + t) * reach]);
      }
      pads.push({ x, y, radius, notch, edge });
    }
    // The cut-out mask grows slightly so pencil outlines and bleed survive.
    this.pads = pads.map((pad) => pad.edge.map(([x, y]) => {
      const d = Math.hypot(x - pad.x, y - pad.y) || 1;
      return [x + ((x - pad.x) / d) * 2, y + ((y - pad.y) / d) * 2] as [number, number];
    }));
    const polygons = function* () { for (const pad of pads) { brush.polygon(pad.edge); yield; } };
    brush.noStroke();
    brush.wash(pal.lily, 225);
    yield* polygons();
    brush.noWash();
    brush.fill(mix(pal.lily, pal.ink, 0.2), 255);
    brush.fillBleed(0.04);
    brush.fillTexture(0.5, 0.8, false);
    yield* polygons();
    yield* polygons();
    brush.fill(mix(pal.lily, pal.paper, 0.55), 200);
    brush.fillBleed(0.15, "in");
    brush.fillTexture(0.4, 0.3, false);
    for (const pad of pads) { brush.circle(pad.x - p.cos(pad.notch) * pad.radius * 0.2, pad.y - p.sin(pad.notch) * pad.radius * 0.2, pad.radius * 0.45, 0.5); yield; }
    brush.noFill();
    brush.set("HB", mix(pal.lily, pal.ink, 0.55), 0.9);
    for (const pad of pads) {
      brush.polygon(pad.edge);
      for (let a = 45; a <= 315; a += 45) {
        brush.line(pad.x, pad.y, pad.x + p.cos(pad.notch + a) * pad.radius * 0.85, pad.y + p.sin(pad.notch + a) * pad.radius * 0.85);
      }
      yield;
    }
    // Lotus blossoms sit on the two largest pads.
    const blooms = [...pads].sort((a, b) => b.radius - a.radius).slice(0, 2).map((pad) => ({ pad, turn: r(0, 60) }));
    const petals = () => { for (const { pad, turn } of blooms) for (let a = 0; a < 360; a += 60) brush.circle(pad.x + p.cos(a + turn) * 10, pad.y + p.sin(a + turn) * 10, 9.5, 0.4); };
    brush.noStroke();
    brush.wash(mix(pal.lotus, "#ffffff", 0.2), 240);
    petals();
    yield;
    brush.noWash();
    brush.fill(mix(pal.lotus, pal.red, 0.3), 255);
    brush.fillBleed(0.05);
    brush.fillTexture(0.4, 0.9, false);
    petals();
    yield;
    brush.noFill();
    brush.wash(mix(pal.lotus, "#ffffff", 0.75), 255);
    for (const { pad } of blooms) brush.circle(pad.x, pad.y, 7, 0.3);
    brush.wash(pal.gold, 255);
    for (const { pad } of blooms) brush.circle(pad.x, pad.y, 3, 0.2);
    brush.noWash();
  }

  // ── Animation (p5 2D) ────────────────────────────────────────────────────────

  private frame(p: p5) {
    const now = performance.now();
    const dt = this.looping ? clamp((now - (this.last || now)) / 1000, 0, 0.1) : 0;
    this.last = now;
    this.time += dt;
    const ctx = p.drawingContext as CanvasRenderingContext2D;
    p.clear();
    p.push();
    p.scale(p.width / SIZE);
    const fade = this.fadeStart < 0 ? 1 : clamp((this.time - this.fadeStart) / 1.4, 0, 1);
    if (fade >= 1) this.fadeStart = -1;
    if (this.painted) {
      if (fade < 1) ctx.drawImage(this.previous, 0, 0, SIZE, SIZE);
      ctx.globalAlpha = fade;
      ctx.drawImage(this.water, 0, 0, SIZE, SIZE);
      ctx.globalAlpha = 1;
    } else {
      // A quiet stand-in until the first painting is ready.
      ctx.fillStyle = mix(this.palette.stone, this.palette.moss, 0.3);
      ctx.beginPath(); ctx.arc(C, C, C - 3, 0, Math.PI * 2); ctx.fill();
      ctx.fillStyle = this.palette.water;
      ctx.beginPath(); ctx.arc(C, C, WATER, 0, Math.PI * 2); ctx.fill();
    }
    this.clipWater(ctx);
    this.dapples(ctx);
    this.update(p, dt);
    for (const fish of this.fish.values()) this.drawFish(ctx, fish, true);
    for (const fish of this.fish.values()) this.drawFish(ctx, fish, false);
    this.drawPellets(ctx);
    if (this.painted) {
      ctx.save();
      const turn = this.reducedMotion ? 0 : this.time * 0.012 + Math.sin(this.time * 0.35) * 0.012;
      ctx.translate(C, C);
      ctx.rotate(turn);
      ctx.translate(-C, -C);
      if (fade < 1) { ctx.globalAlpha = 1 - fade; ctx.drawImage(this.previousLilies, 0, 0, SIZE, SIZE); }
      ctx.globalAlpha = fade;
      ctx.drawImage(this.lilies, 0, 0, SIZE, SIZE);
      ctx.restore();
    }
    this.drawRipples(ctx);
    this.sparkles(ctx);
    ctx.restore();
    this.nameTag(ctx, p);
    p.pop();
  }

  private clipWater(ctx: CanvasRenderingContext2D) {
    ctx.save();
    ctx.beginPath();
    ctx.arc(C, C, WATER - 3, 0, Math.PI * 2);
    ctx.clip();
  }

  private dapples(ctx: CanvasRenderingContext2D) {
    ctx.save();
    ctx.globalCompositeOperation = "screen";
    for (let i = 0; i < 5; i++) {
      const x = C + Math.sin(this.time * 0.07 + i * 2.1) * 150, y = C + Math.cos(this.time * 0.05 + i * 1.3) * 140;
      const glow = ctx.createRadialGradient(x, y, 0, x, y, 90);
      glow.addColorStop(0, alpha(this.palette.shallow, 0.16));
      glow.addColorStop(1, alpha(this.palette.shallow, 0));
      ctx.fillStyle = glow;
      ctx.fillRect(x - 90, y - 90, 180, 180);
    }
    ctx.restore();
  }

  private update(p: p5, dt: number) {
    if (!dt) return;
    const all = [...this.fish.values()];
    for (const fish of all) {
      fish.fade = clamp(fish.fade + (fish.leaving ? -dt : dt) / 1.2, 0, 1);
      if (fish.leaving && fish.fade === 0) { this.fish.delete(fish.spec.id); continue; }
      fish.gulp = Math.max(0, fish.gulp - dt * 2.5);
      const head = fish.head;
      let turn = (p.noise(fish.noise, this.time * 0.18) - 0.5) * 2.4;
      let target = 14 + p.noise(fish.noise + 50, this.time * 0.1) * 14;
      const distance = Math.hypot(head.x - C, head.y - C);
      const lookX = head.x + Math.cos(fish.heading) * 40, lookY = head.y + Math.sin(fish.heading) * 40;
      if (Math.hypot(lookX - C, lookY - C) > SWIM) {
        const home = Math.atan2(C - head.y, C - head.x);
        turn += clamp(wrap(home - fish.heading), -1, 1) * (1.4 + (distance - SWIM * 0.8) / 30);
      }
      const food = this.pellets.filter((pellet) => pellet.born <= this.time).reduce<Pellet | undefined>(
        (best, pellet) => (!best || Math.hypot(pellet.x - head.x, pellet.y - head.y) < Math.hypot(best.x - head.x, best.y - head.y) ? pellet : best), undefined);
      if (food && !fish.leaving) {
        const d = Math.hypot(food.x - head.x, food.y - head.y);
        turn = clamp(wrap(Math.atan2(food.y - head.y, food.x - head.x) - fish.heading) * 3, -2.6, 2.6);
        target = clamp(d * 0.6, 22, 58);
        if (d < 9 * fish.spec.size + 3) {
          this.pellets.splice(this.pellets.indexOf(food), 1);
          fish.gulp = 1;
          this.ripple(food.x, food.y, 0.45);
        }
      }
      for (const other of all) {
        if (other === fish) continue;
        const d = Math.hypot(other.head.x - head.x, other.head.y - head.y);
        if (d < 36 && d > 0) turn += clamp(wrap(fish.heading - Math.atan2(other.head.y - head.y, other.head.x - head.x)), -1, 1) * (36 - d) / 36 * 0.8;
      }
      fish.heading = wrap(fish.heading + clamp(turn, -2.8, 2.8) * dt);
      fish.speed += (target - fish.speed) * Math.min(1, dt * 1.5);
      head.x += Math.cos(fish.heading) * fish.speed * dt;
      head.y += Math.sin(fish.heading) * fish.speed * dt;
      for (let i = 1; i < fish.spine.length; i++) {
        const a = fish.spine[i - 1]!, b = fish.spine[i]!;
        const angle = Math.atan2(b.y - a.y, b.x - a.x);
        b.x = a.x + Math.cos(angle) * fish.segment;
        b.y = a.y + Math.sin(angle) * fish.segment;
      }
      fish.phase += dt * (2.2 + fish.speed * 0.14);
    }
    this.pellets = this.pellets.filter((pellet) => this.time - pellet.born < 24);
    if (this.time > this.nextAmbient) {
      const a = Math.random() * Math.PI * 2, d = Math.sqrt(Math.random()) * (WATER - 30);
      this.ripple(C + Math.cos(a) * d, C + Math.sin(a) * d, 0.35 + Math.random() * 0.3);
      // Now and then a koi pokes up for a little bloop.
      const fish = all[Math.floor(Math.random() * all.length)];
      if (fish && Math.random() < 0.35) this.ripple(fish.head.x, fish.head.y, 0.5);
      this.nextAmbient = this.time + 3 + Math.random() * 6;
    }
    this.ripples = this.ripples.filter((ripple) => this.time - ripple.born < 3);
  }

  private body(fish: Fish) {
    const left: [number, number][] = [], right: [number, number][] = [];
    const n = fish.spine.length - 1;
    const wave = this.reducedMotion ? 0 : 1;
    const points = fish.spine.map((point, i) => {
      const u = i / n;
      const next = fish.spine[Math.min(n, i + 1)]!, prev = fish.spine[Math.max(0, i - 1)]!;
      const angle = Math.atan2(prev.y - next.y, prev.x - next.x);
      const sway = Math.sin(fish.phase - u * 4.2) * fish.length * 0.045 * u ** 1.4 * wave;
      return { x: point.x - Math.sin(angle) * sway, y: point.y + Math.cos(angle) * sway, angle, u };
    });
    const bulge = 1 + fish.gulp * 0.08;
    for (const point of points) {
      const w = fish.length * 0.13 * bulge * (point.u < 0.25 ? 0.72 + Math.sin((point.u / 0.25) * Math.PI / 2) * 0.28 : Math.max(0.12, 1 - ((point.u - 0.25) / 0.75) ** 1.3 * 0.9));
      left.push([point.x - Math.sin(point.angle) * w, point.y + Math.cos(point.angle) * w]);
      right.push([point.x + Math.sin(point.angle) * w, point.y - Math.cos(point.angle) * w]);
    }
    return { points, left, right };
  }

  private drawFish(ctx: CanvasRenderingContext2D, fish: Fish, shadow: boolean) {
    if (!fish.fade) return;
    const { points, left, right } = this.body(fish);
    const head = points[0]!, tail = points[points.length - 1]!;
    const path = new Path2D();
    const nose: [number, number] = [head.x + Math.cos(head.angle) * fish.length * 0.07, head.y + Math.sin(head.angle) * fish.length * 0.07];
    path.moveTo(...nose);
    path.quadraticCurveTo(left[0]![0] + Math.cos(head.angle) * fish.length * 0.05, left[0]![1] + Math.sin(head.angle) * fish.length * 0.05, left[0]![0], left[0]![1]);
    for (let i = 1; i < left.length; i++) {
      const [ax, ay] = left[i - 1]!, [bx, by] = left[i]!;
      path.quadraticCurveTo(ax, ay, (ax + bx) / 2, (ay + by) / 2);
    }
    path.lineTo(...left[left.length - 1]!);
    path.lineTo(...right[right.length - 1]!);
    for (let i = right.length - 2; i >= 0; i--) {
      const [ax, ay] = right[i + 1]!, [bx, by] = right[i]!;
      path.quadraticCurveTo(ax, ay, (ax + bx) / 2, (ay + by) / 2);
    }
    path.quadraticCurveTo(right[0]![0] + Math.cos(head.angle) * fish.length * 0.05, right[0]![1] + Math.sin(head.angle) * fish.length * 0.05, ...nose);
    path.closePath();

    // Tail fan and pectoral fins, flapping with the swim phase.
    const fins = new Path2D();
    const flap = Math.sin(fish.phase * 1.3) * 0.25;
    const back = tail.angle + Math.PI;
    const spread = 0.55 + Math.sin(fish.phase) * 0.12;
    const tailLength = fish.length * 0.26;
    const tip = (offset: number): [number, number] => [tail.x + Math.cos(back + offset) * tailLength, tail.y + Math.sin(back + offset) * tailLength];
    const [lx, ly] = tip(spread), [rx, ry] = tip(-spread), [mx, my] = tip(0);
    fins.moveTo(tail.x, tail.y);
    fins.quadraticCurveTo(tail.x + (lx - tail.x) * 0.5 + Math.cos(back + spread + 0.6) * 6, tail.y + (ly - tail.y) * 0.5 + Math.sin(back + spread + 0.6) * 6, lx, ly);
    fins.quadraticCurveTo(mx + (tail.x - mx) * 0.3, my + (tail.y - my) * 0.3, rx, ry);
    fins.quadraticCurveTo(tail.x + (rx - tail.x) * 0.5 + Math.cos(back - spread - 0.6) * 6, tail.y + (ry - tail.y) * 0.5 + Math.sin(back - spread - 0.6) * 6, tail.x, tail.y);
    const pec = points[3]!;
    for (const side of [1, -1]) {
      const w = fish.length * 0.12;
      const angle = pec.angle + Math.PI + side * (1.05 + flap);
      const bx = pec.x + Math.cos(pec.angle + side * Math.PI / 2) * w * 0.8, by = pec.y + Math.sin(pec.angle + side * Math.PI / 2) * w * 0.8;
      fins.moveTo(bx, by);
      fins.ellipse(bx + Math.cos(angle) * w * 0.55, by + Math.sin(angle) * w * 0.55, w * 0.62, w * 0.3, angle, 0, Math.PI * 2);
    }

    ctx.save();
    ctx.globalAlpha = fish.fade;
    if (shadow) {
      ctx.translate(7, 10);
      ctx.fillStyle = alpha(this.palette.ink, 0.16);
      ctx.fill(path);
      ctx.fill(fins);
      ctx.restore();
      return;
    }
    const { base, fin, spots } = fish.colors;
    ctx.fillStyle = alpha(fin, 0.62);
    ctx.fill(fins);
    ctx.strokeStyle = alpha(mix(fin, this.palette.ink, 0.35), 0.35);
    ctx.lineWidth = 0.8;
    ctx.stroke(fins);
    ctx.fillStyle = base;
    ctx.fill(path);
    ctx.save();
    ctx.clip(path);
    for (const spot of spots) {
      const at = clamp(spot.u, 0, 1) * (points.length - 1);
      const a = points[Math.floor(at)]!, b = points[Math.min(points.length - 1, Math.ceil(at))]!, k = at - Math.floor(at);
      const x = a.x + (b.x - a.x) * k, y = a.y + (b.y - a.y) * k;
      const w = fish.length * 0.13 * spot.v;
      ctx.fillStyle = spot.color;
      ctx.beginPath();
      ctx.ellipse(x - Math.sin(a.angle) * w, y + Math.cos(a.angle) * w, fish.length * 0.1 * spot.r * 1.25, fish.length * 0.1 * spot.r, a.angle, 0, Math.PI * 2);
      ctx.fill();
    }
    // A soft sheen along the back.
    ctx.strokeStyle = "rgba(255,255,255,0.28)";
    ctx.lineWidth = fish.length * 0.05;
    ctx.lineCap = "round";
    ctx.beginPath();
    ctx.moveTo(points[1]!.x, points[1]!.y);
    for (let i = 2; i < 8; i++) ctx.lineTo(points[i]!.x, points[i]!.y);
    ctx.stroke();
    ctx.restore();
    ctx.strokeStyle = alpha(mix(base, this.palette.ink, 0.5), 0.4);
    ctx.lineWidth = 1;
    ctx.stroke(path);

    // Two little eyes and rosy cheeks.
    const eye = points[1]!;
    for (const side of [1, -1]) {
      const w = fish.length * 0.085;
      const ex = eye.x + Math.cos(eye.angle + side * Math.PI / 2) * w, ey = eye.y + Math.sin(eye.angle + side * Math.PI / 2) * w;
      ctx.fillStyle = alpha(this.palette.lotus, 0.45);
      ctx.beginPath();
      ctx.ellipse(ex - Math.cos(eye.angle) * fish.length * 0.05, ey - Math.sin(eye.angle) * fish.length * 0.05, fish.length * 0.03, fish.length * 0.02, eye.angle, 0, Math.PI * 2);
      ctx.fill();
      ctx.fillStyle = "#1d1d22";
      ctx.beginPath();
      ctx.arc(ex, ey, fish.length * 0.028, 0, Math.PI * 2);
      ctx.fill();
      ctx.fillStyle = "#ffffff";
      ctx.beginPath();
      ctx.arc(ex + Math.cos(eye.angle) * fish.length * 0.01, ey + Math.sin(eye.angle) * fish.length * 0.01, fish.length * 0.009, 0, Math.PI * 2);
      ctx.fill();
    }
    ctx.restore();
  }

  private drawPellets(ctx: CanvasRenderingContext2D) {
    for (const pellet of this.pellets) {
      const age = this.time - pellet.born;
      if (age < 0) continue;
      const bob = Math.sin(age * 3 + pellet.x) * 0.8;
      ctx.globalAlpha = clamp(Math.min(age * 4, (24 - age) / 3), 0, 1);
      ctx.fillStyle = mix(this.palette.orange, this.palette.ink, 0.35);
      ctx.beginPath();
      ctx.arc(pellet.x, pellet.y + bob, 3.2, 0, Math.PI * 2);
      ctx.fill();
      ctx.fillStyle = "rgba(255,255,255,0.5)";
      ctx.beginPath();
      ctx.arc(pellet.x - 1, pellet.y + bob - 1, 1, 0, Math.PI * 2);
      ctx.fill();
    }
    ctx.globalAlpha = 1;
  }

  private drawRipples(ctx: CanvasRenderingContext2D) {
    ctx.lineWidth = 1.4;
    for (const ripple of this.ripples) {
      const age = this.time - ripple.born;
      for (const [delay, scale] of [[0, 1], [0.35, 0.7]] as const) {
        const t = age - delay;
        if (t <= 0) continue;
        const life = 2.4 * ripple.strength + 0.6;
        if (t > life) continue;
        ctx.strokeStyle = `rgba(255,255,255,${(1 - t / life) * 0.55 * scale})`;
        ctx.beginPath();
        ctx.ellipse(ripple.x, ripple.y, t * 26 * ripple.strength + 3, (t * 26 * ripple.strength + 3) * 0.92, 0, 0, Math.PI * 2);
        ctx.stroke();
      }
    }
  }

  private sparkles(ctx: CanvasRenderingContext2D) {
    if (this.reducedMotion) return;
    const rand = mulberry(this.seed);
    for (let i = 0; i < 6; i++) {
      const a = rand() * Math.PI * 2, d = Math.sqrt(rand()) * (WATER - 40);
      const twinkle = Math.sin(this.time * (0.6 + rand()) + rand() * 10);
      if (twinkle < 0.55) continue;
      const x = C + Math.cos(a) * d, y = C + Math.sin(a) * d, s = (twinkle - 0.55) * 11;
      ctx.fillStyle = `rgba(255,255,255,${(twinkle - 0.55) * 1.6})`;
      ctx.beginPath();
      ctx.moveTo(x, y - s); ctx.quadraticCurveTo(x, y, x + s, y); ctx.quadraticCurveTo(x, y, x, y + s);
      ctx.quadraticCurveTo(x, y, x - s, y); ctx.quadraticCurveTo(x, y, x, y - s);
      ctx.fill();
    }
  }

  private nameTag(ctx: CanvasRenderingContext2D, p: p5) {
    if (!this.hover) return;
    const scale = SIZE / p.width;
    const hx = this.hover.x * scale, hy = this.hover.y * scale;
    let best: Fish | undefined, distance = 34;
    for (const fish of this.fish.values()) {
      if (fish.leaving) continue;
      for (const point of fish.spine.slice(0, 8)) {
        const d = Math.hypot(point.x - hx, point.y - hy);
        if (d < distance) { distance = d; best = fish; }
      }
    }
    if (!best || !best.spec.name.trim()) return;
    const name = best.spec.name.trim().slice(0, 24);
    const x = best.head.x, y = best.head.y - 26;
    ctx.font = `600 13px "Avenir Next", Avenir, system-ui, sans-serif`;
    const w = ctx.measureText(name).width + 18;
    ctx.fillStyle = alpha(this.palette.paper, 0.92);
    ctx.beginPath();
    ctx.roundRect(x - w / 2, y - 12, w, 24, 12);
    ctx.fill();
    ctx.fillStyle = this.palette.ink;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText(name, x, y + 0.5);
  }
}
