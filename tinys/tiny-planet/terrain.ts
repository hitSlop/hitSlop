// Pure terrain: the planet's shape is a function of a seed, so only the seed is saved.
export const SEA = 1;

function hash(x: number, y: number, z: number, seed: number): number {
  let h = Math.imul(x, 374761393) + Math.imul(y, 668265263) + Math.imul(z, 1442695041) + Math.imul(seed, 2246822519);
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  h ^= h >>> 16;
  return (h >>> 0) / 4294967295;
}

const smooth = (t: number) => t * t * (3 - 2 * t);
const mix = (a: number, b: number, t: number) => a + (b - a) * t;

function valueNoise(x: number, y: number, z: number, seed: number): number {
  const ix = Math.floor(x), iy = Math.floor(y), iz = Math.floor(z);
  const fx = smooth(x - ix), fy = smooth(y - iy), fz = smooth(z - iz);
  const corner = (dx: number, dy: number, dz: number) => hash(ix + dx, iy + dy, iz + dz, seed);
  return mix(
    mix(mix(corner(0, 0, 0), corner(1, 0, 0), fx), mix(corner(0, 1, 0), corner(1, 1, 0), fx), fy),
    mix(mix(corner(0, 0, 1), corner(1, 0, 1), fx), mix(corner(0, 1, 1), corner(1, 1, 1), fx), fy),
    fz,
  );
}

/** Surface radius in the direction (x, y, z), which must be a unit vector. Sea level is 1. */
export function radiusAt(x: number, y: number, z: number, seed: number): number {
  const n = valueNoise(x * 1.7 + 11, y * 1.7 + 11, z * 1.7 + 11, seed) * 0.65 + valueNoise(x * 3.9 + 5, y * 3.9 + 5, z * 3.9 + 5, seed + 7) * 0.35;
  return 1 + (n - 0.5) * 0.3;
}

export const direction = (lat: number, lon: number): [number, number, number] =>
  [Math.cos(lat) * Math.cos(lon), Math.sin(lat), Math.cos(lat) * Math.sin(lon)];

export const coordinates = (x: number, y: number, z: number): { lat: number; lon: number } => ({
  lat: Math.round(Math.asin(Math.max(-1, Math.min(1, y))) * 1e4) / 1e4,
  lon: Math.round(Math.atan2(z, x) * 1e4) / 1e4,
});
