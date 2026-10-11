import { gsap } from "gsap";

const CHARSET = " ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789:./-";
const FLIP = 0.05;
const MAX_STEPS = 9;

export interface FlapOptions {
  char: string;
  delay: number;
  instant: boolean;
  onflip?: () => void;
}

/** The letters a flap rolls through to land on `to`: at most MAX_STEPS, the last one being `to`. */
function path(from: string, to: string): string[] {
  const a = CHARSET.indexOf(from);
  const b = CHARSET.indexOf(to);
  if (b < 0 || a < 0) return from === to ? [] : [to];
  const distance = (b - a + CHARSET.length) % CHARSET.length;
  const count = Math.min(distance, MAX_STEPS);
  return Array.from({ length: count }, (_, i) => CHARSET[(b - count + 1 + i + CHARSET.length) % CHARSET.length]!);
}

/** One split-flap character. It builds its own four layers and lets GSAP rotate the halves. */
export function flap(node: HTMLElement, options: FlapOptions) {
  const layer = (kind: "top" | "bottom", flipping: boolean) => {
    const half = document.createElement("span");
    half.className = `half ${kind}${flipping ? " flip" : ""}`;
    const glyph = document.createElement("i");
    half.append(glyph);
    node.append(half);
    return { half, glyph };
  };
  const top = layer("top", false);
  const bottom = layer("bottom", false);
  const flipTop = layer("top", true);
  const flipBottom = layer("bottom", true);

  let options_ = options;
  let target = " ";
  let timeline: gsap.core.Timeline | undefined;

  const write = (upper: string, lower: string) => {
    top.glyph.textContent = upper;
    bottom.glyph.textContent = lower;
  };
  const rest = () => {
    gsap.set([flipTop.half, flipBottom.half], { visibility: "hidden", rotationX: 0 });
    write(target, target);
  };

  function play(to: string) {
    timeline?.kill();
    rest();
    const letters = options_.instant ? [] : path(target, to);
    if (!letters.length) {
      target = to;
      rest();
      return;
    }
    let previous = target;
    target = to;
    const tl = gsap.timeline({ delay: options_.delay, onComplete: rest });
    for (const next of letters) {
      const from = previous;
      tl.add(() => {
        write(next, from);
        flipTop.glyph.textContent = from;
        flipBottom.glyph.textContent = next;
        gsap.set(flipTop.half, { visibility: "visible", rotationX: 0 });
        gsap.set(flipBottom.half, { visibility: "visible", rotationX: 90 });
        options_.onflip?.();
      });
      tl.to(flipTop.half, { rotationX: -90, duration: FLIP, ease: "power1.in" });
      tl.to(flipBottom.half, { rotationX: 0, duration: FLIP, ease: "power1.out" });
      tl.add(() => { bottom.glyph.textContent = next; gsap.set([flipTop.half, flipBottom.half], { visibility: "hidden" }); });
      previous = next;
    }
    timeline = tl;
  }

  rest();
  // A new board starts blank and clacks into place.
  play(options.char);

  return {
    update(next: FlapOptions) {
      options_ = next;
      if (next.char !== target) play(next.char);
    },
    destroy() {
      timeline?.kill();
    },
  };
}
