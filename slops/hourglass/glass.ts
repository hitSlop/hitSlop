// The sand glass's geometry, in window points at its initial size. `slop.ts` reads the
// silhouette for the window's shape and the app draws the same outline, so the frost, the
// painted vessel and the sand always agree.

export const width = 280;
export const height = 460;
/** The walnut caps' height, top and bottom. */
export const cap = 46;
export const neck = height / 2;
const center = width / 2;

/** The top bulb's right wall, from the cap to the neck, as cubic Bézier points. */
const wall = [
  [246, cap - 2],
  [262, 170],
  [176, 222],
  [center + 6, neck],
] as const;
const [p0, p1, p2, p3] = wall;

const mirrorX = (x: number) => width - x;
const mirrorY = (y: number) => height - y;

/** The glass between the caps: both bulbs and the neck, clockwise. */
export const body = [
  `M${mirrorX(p0[0])} ${p0[1]} H${p0[0]}`,
  `C${p1[0]} ${p1[1]} ${p2[0]} ${p2[1]} ${p3[0]} ${p3[1]}`,
  `C${p2[0]} ${mirrorY(p2[1])} ${p1[0]} ${mirrorY(p1[1])} ${p0[0]} ${mirrorY(p0[1])}`,
  `H${mirrorX(p0[0])}`,
  `C${mirrorX(p1[0])} ${mirrorY(p1[1])} ${mirrorX(p2[0])} ${mirrorY(p2[1])} ${mirrorX(p3[0])} ${p3[1]}`,
  `C${mirrorX(p2[0])} ${p2[1]} ${mirrorX(p1[0])} ${p1[1]} ${mirrorX(p0[0])} ${p0[1]} Z`,
].join(" ");

const radius = 14;
const capPath = (top: number) =>
  `M${radius} ${top} H${width - radius} A${radius} ${radius} 0 0 1 ${width} ${top + radius} ` +
  `V${top + cap - radius} A${radius} ${radius} 0 0 1 ${width - radius} ${top + cap} H${radius} ` +
  `A${radius} ${radius} 0 0 1 0 ${top + cap - radius} V${top + radius} A${radius} ${radius} 0 0 1 ${radius} ${top} Z`;
export const caps = [capPath(0), capPath(height - cap)];

/** The two posts between the caps, left and right. */
export const posts = [10, width - 18].map((x) => ({ x, y: cap - 4, width: 8, height: height - 2 * cap + 8 }));
const postPath = ({ x, y, width, height }: (typeof posts)[number]) => `M${x} ${y} H${x + width} V${y + height} H${x} Z`;

/** The window's outline: caps, posts and glass. The gaps between the posts and the glass
 * are holes in the window, so the desktop shows through them and clicks reach it. */
export const silhouette = [...caps, ...posts.map(postPath), body].join(" ");

// The wall's half-width down the top bulb, sampled once; the bottom bulb mirrors it.
const samples = Array.from({ length: 241 }, (_, i) => {
  const t = i / 240;
  const u = 1 - t;
  const at = (k: 0 | 1) => u * u * u * p0[k] + 3 * u * u * t * p1[k] + 3 * u * t * t * p2[k] + t * t * t * p3[k];
  return { y: at(1), half: at(0) - center };
});
/** The glass's cross-section from the top bulb's top edge down to `y`. */
function areaTo(y: number): number {
  let area = 0;
  for (let i = 1; i < samples.length && samples[i - 1].y < y; i++) {
    const a = samples[i - 1];
    const b = samples[i].y > y ? { y, half: a.half + ((samples[i].half - a.half) * (y - a.y)) / (samples[i].y - a.y) } : samples[i];
    area += (b.y - a.y) * (a.half + b.half);
  }
  return area;
}
/** The glass's half-width at `y` in the top bulb. */
function halfAt(y: number): number {
  let low = 0;
  let high = samples.length - 1;
  while (high - low > 1) {
    const middle = (low + high) >> 1;
    if (samples[middle].y < y) low = middle;
    else high = middle;
  }
  const [a, b] = [samples[low], samples[high]];
  return a.half + ((b.half - a.half) * (Math.min(Math.max(y, a.y), b.y) - a.y)) / (b.y - a.y || 1);
}
/** The `x` between `low` and `high` where `f`, which grows with `x`, reaches `target`. */
function solve(f: (x: number) => number, target: number, low: number, high: number): number {
  for (let i = 0; i < 32; i++) {
    const middle = (low + high) / 2;
    if (f(middle) < target) low = middle;
    else high = middle;
  }
  return (low + high) / 2;
}

/** Where a full glass's sand stops in the top bulb, leaving room for the readout above it. */
const fullLevel = 176;
const sandArea = areaTo(neck) - areaTo(fullLevel);
/** The bottom bulb's floor, where the pile stands. */
export const base = mirrorY(p0[1]);
/** The pile's slope, rise over run: fallen sand heaps under the stream at its angle of
 * repose instead of spreading flat, so the first minutes of sand are already a mound. */
export const repose = 0.6;
/** A pile `rise` tall: a cone under the neck, cut off by the glass once it reaches the walls. */
function pileArea(rise: number): number {
  const peak = base - rise;
  const steps = 48;
  let area = 0;
  for (let i = 0; i < steps; i++) {
    const y = peak + ((i + 0.5) * rise) / steps;
    area += (Math.min(2 * halfAt(mirrorY(y)), (2 * (y - peak)) / repose) * rise) / steps;
  }
  return area;
}

/** Where the sand stands with `remaining` (0 to 1) of it still in the top bulb: the level
 * of the top bulb's surface, and the peak of the pile below. The two hold the same sand. */
export function sandLevels(remaining: number) {
  const left = Math.min(Math.max(remaining, 0), 1);
  return {
    top: solve(areaTo, areaTo(neck) - left * sandArea, p0[1], neck),
    peak: base - solve(pileArea, (1 - left) * sandArea, 0, base - neck),
  };
}
