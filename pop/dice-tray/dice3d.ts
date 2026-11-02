import * as THREE from "three";
import type { Kind } from "./schema";

// The 3D dice are built here from document data (kind, value, seed) and thrown away with the
// view. Nothing in this file decides a result: values come from the saved roll, and the seed
// only shapes the tumble and where each die comes to rest.

/** The saved roll as the tray shows it. */
export interface ShownRoll { id: string; seed: number; dice: readonly { kind: Kind; value: number }[] }

export const numeralFont = '"Dice Tray Serif"';

export interface Palette { die: string; pip: string; felt: string; feltDeep: string }
const paletteTokens = ["die", "pip", "felt", "feltDeep"] as const;

/** Theme colors as styles.css hands them to the scene (`--tray-*` on the element). */
export function readPalette(element: Element): Palette {
  const style = getComputedStyle(element);
  return Object.fromEntries(paletteTokens.map((token) => [token, style.getPropertyValue(`--tray-${token}`).trim() || "#888888"])) as unknown as Palette;
}

/** A small seeded generator (mulberry32): the same seed always tumbles the same way. */
export function seeded(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

interface Face {
  normal: THREE.Vector3;
  center: THREE.Vector3;
  up: THREE.Vector3;
  right: THREE.Vector3;
  radius: number;
  vertices: THREE.Vector3[];
  value: number;
  /** d4 only: the value printed at each of this face's corners. */
  corners?: { at: THREE.Vector3; value: number }[];
}

interface Shape {
  kind: Kind;
  geometry: THREE.BufferGeometry;
  faces: Face[];
  /** Distance from the centre to the farthest vertex, for clearing the felt mid-tumble. */
  reach: number;
  cols: number;
  rows: number;
}

/** Overall size of each kind relative to the others, as they sit side by side in a real tray. */
const sizeOf: Record<Kind, number> = { d4: 1.18, d6: 0.92, d8: 1.0, d10: 1.0, d12: 1.02, d20: 1.08 };

function pentagonalTrapezohedron(): THREE.BufferGeometry {
  // Ten kites: each ring vertex sits slightly above or below the equator, and the poles are
  // placed so every kite is exactly planar.
  const e = 0.1;
  const cos36 = Math.cos(Math.PI / 5);
  const h = (e * (1 + cos36)) / (1 - cos36);
  const top = new THREE.Vector3(0, h, 0);
  const bottom = new THREE.Vector3(0, -h, 0);
  const ring = Array.from({ length: 10 }, (_, i) => new THREE.Vector3(Math.cos((i * Math.PI) / 5), i % 2 ? -e : e, Math.sin((i * Math.PI) / 5)));
  const at = (i: number) => ring[((i % 10) + 10) % 10]!;
  const triangles: THREE.Vector3[][] = [];
  for (let k = 0; k < 5; k++) {
    triangles.push([top, at(2 * k), at(2 * k + 1)], [top, at(2 * k + 1), at(2 * k + 2)]);
    triangles.push([bottom, at(2 * k + 1), at(2 * k + 2)], [bottom, at(2 * k + 2), at(2 * k + 3)]);
  }
  const positions: number[] = [];
  for (const [a, b, c] of triangles) {
    // Wind every triangle outward.
    const normal = new THREE.Vector3().crossVectors(b!.clone().sub(a!), c!.clone().sub(a!));
    const centroid = a!.clone().add(b!).add(c!);
    const order = normal.dot(centroid) >= 0 ? [a!, b!, c!] : [a!, c!, b!];
    for (const v of order) positions.push(v.x, v.y, v.z);
  }
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
  return geometry;
}

function baseGeometry(kind: Kind): THREE.BufferGeometry {
  switch (kind) {
    case "d4": return new THREE.TetrahedronGeometry(1, 0);
    case "d6": return new THREE.BoxGeometry(1, 1, 1).toNonIndexed();
    case "d8": return new THREE.OctahedronGeometry(1, 0);
    case "d10": return pentagonalTrapezohedron();
    case "d12": return new THREE.DodecahedronGeometry(1, 0);
    case "d20": return new THREE.IcosahedronGeometry(1, 0);
  }
}

const keyOf = (v: THREE.Vector3, digits = 3) => `${v.x.toFixed(digits)},${v.y.toFixed(digits)},${v.z.toFixed(digits)}`;

function buildShape(kind: Kind): Shape {
  const geometry = baseGeometry(kind);
  geometry.deleteAttribute("uv");
  geometry.deleteAttribute("normal");
  const position = geometry.getAttribute("position") as THREE.BufferAttribute;
  // Scale so the farthest vertex sits at this kind's size.
  let far = 0;
  const v = new THREE.Vector3();
  for (let i = 0; i < position.count; i++) far = Math.max(far, v.fromBufferAttribute(position, i).length());
  geometry.scale(sizeOf[kind] / far, sizeOf[kind] / far, sizeOf[kind] / far);

  // Group triangles into flat faces by their normal.
  type Group = { normal: THREE.Vector3; triangles: number[]; vertices: Map<string, THREE.Vector3> };
  const groups = new Map<string, Group>();
  const a = new THREE.Vector3(), b = new THREE.Vector3(), c = new THREE.Vector3();
  for (let t = 0; t < position.count / 3; t++) {
    a.fromBufferAttribute(position, t * 3); b.fromBufferAttribute(position, t * 3 + 1); c.fromBufferAttribute(position, t * 3 + 2);
    const normal = new THREE.Vector3().crossVectors(b.clone().sub(a), c.clone().sub(a)).normalize();
    const key = keyOf(normal, 2);
    const group: Group = groups.get(key) ?? { normal, triangles: [], vertices: new Map() };
    group.triangles.push(t);
    for (const p of [a, b, c]) group.vertices.set(keyOf(p), p.clone());
    groups.set(key, group);
  }

  const faces: (Face & { triangles: number[] })[] = [...groups.values()].map((group) => {
    const vertices = [...group.vertices.values()];
    const center = vertices.reduce((sum, p) => sum.add(p), new THREE.Vector3()).divideScalar(vertices.length);
    const normal = group.normal;
    const radius = Math.max(...vertices.map((p) => p.distanceTo(center)));
    const flat = (dir: THREE.Vector3) => dir.clone().sub(normal.clone().multiplyScalar(dir.dot(normal))).normalize();
    let up: THREE.Vector3;
    if (kind === "d6") {
      up = flat(Math.abs(normal.y) > 0.9 ? new THREE.Vector3(0, 0, -1) : new THREE.Vector3(0, 1, 0));
    } else if (kind === "d10") {
      // Numerals point at the pole, as on a real d10.
      const apex = vertices.reduce((best, p) => (p.distanceTo(center) > best.distanceTo(center) ? p : best));
      up = flat(apex.clone().sub(center));
    } else {
      const toward = flat(Math.abs(normal.y) > 0.9 ? new THREE.Vector3(0, 0, -1) : new THREE.Vector3(0, 1, 0));
      const corner = vertices.reduce((best, p) => (p.clone().sub(center).dot(toward) > best.clone().sub(center).dot(toward) ? p : best));
      up = flat(corner.clone().sub(center));
    }
    const right = new THREE.Vector3().crossVectors(up, normal).normalize();
    return { normal, center, up, right, radius, vertices, value: 0, triangles: group.triangles };
  });

  // A stable order, then values: opposite faces sum to sides + 1 like real dice.
  faces.sort((p, q) => q.normal.y - p.normal.y || Math.atan2(p.normal.z, p.normal.x) - Math.atan2(q.normal.z, q.normal.x));
  if (kind === "d4") {
    const corners = new Map<string, { at: THREE.Vector3; value: number }>();
    for (const face of faces) for (const p of face.vertices) if (!corners.has(keyOf(p))) corners.set(keyOf(p), { at: p, value: corners.size + 1 });
    for (const face of faces) {
      face.corners = face.vertices.map((p) => corners.get(keyOf(p))!);
      // The face is identified by the corner it lacks: resting on it shows that number on top.
      face.value = [...corners.values()].find((corner) => !face.vertices.some((p) => keyOf(p) === keyOf(corner.at)))!.value;
    }
  } else {
    const total = faces.length + 1;
    let next = 1;
    for (const face of faces) {
      if (face.value) continue;
      const opposite = faces.find((other) => other !== face && !other.value && other.normal.dot(face.normal) < -0.99);
      face.value = next;
      if (opposite) opposite.value = total - next;
      next++;
    }
  }

  // UVs: each face gets its own cell of the numeral atlas.
  const cols = Math.ceil(Math.sqrt(faces.length));
  const rows = Math.ceil(faces.length / cols);
  const uv = new Float32Array(position.count * 2);
  faces.forEach((face, index) => {
    const col = index % cols, row = Math.floor(index / cols);
    for (const t of face.triangles) {
      for (let k = 0; k < 3; k++) {
        const p = new THREE.Vector3().fromBufferAttribute(position, t * 3 + k).sub(face.center);
        const x = 0.5 + (p.dot(face.right) / face.radius) * 0.47;
        const y = 0.5 + (p.dot(face.up) / face.radius) * 0.47;
        uv[(t * 3 + k) * 2] = (col + x) / cols;
        uv[(t * 3 + k) * 2 + 1] = (rows - row - 1 + y) / rows;
      }
    }
  });
  geometry.setAttribute("uv", new THREE.BufferAttribute(uv, 2));
  geometry.computeVertexNormals();
  return { kind, geometry, faces, reach: sizeOf[kind], cols, rows };
}

const CELL = 160;

function drawAtlas(canvas: HTMLCanvasElement, shape: Shape, palette: Palette): void {
  canvas.width = shape.cols * CELL;
  canvas.height = shape.rows * CELL;
  const g = canvas.getContext("2d")!;
  g.fillStyle = palette.die;
  g.fillRect(0, 0, canvas.width, canvas.height);
  g.fillStyle = palette.pip;
  g.textAlign = "center";
  g.textBaseline = "middle";
  const scale = (CELL * 0.47) / 1; // local face units (radius = 1) to pixels
  shape.faces.forEach((face, index) => {
    const cx = (index % shape.cols) * CELL + CELL / 2;
    const cy = Math.floor(index / shape.cols) * CELL + CELL / 2;
    g.save();
    g.translate(cx, cy);
    if (shape.kind === "d6") {
      const r = CELL * 0.065, d = CELL * 0.19;
      const spots: Record<number, [number, number][]> = {
        1: [[0, 0]], 2: [[-1, -1], [1, 1]], 3: [[-1, -1], [0, 0], [1, 1]], 4: [[-1, -1], [1, -1], [-1, 1], [1, 1]],
        5: [[-1, -1], [1, -1], [0, 0], [-1, 1], [1, 1]], 6: [[-1, -1], [1, -1], [-1, 0], [1, 0], [-1, 1], [1, 1]],
      };
      for (const [x, y] of spots[face.value] ?? []) {
        g.beginPath();
        g.arc(x * d, y * d, face.value === 1 ? r * 1.5 : r, 0, Math.PI * 2);
        g.fill();
      }
    } else if (shape.kind === "d4" && face.corners) {
      g.font = `700 ${Math.round(CELL * 0.2)}px ${numeralFont}, Georgia, serif`;
      for (const corner of face.corners) {
        const p = corner.at.clone().sub(face.center);
        const x = (p.dot(face.right) / face.radius) * scale * 0.56;
        const y = -(p.dot(face.up) / face.radius) * scale * 0.56;
        g.save();
        g.translate(x, y);
        g.rotate(Math.atan2(x, -y));
        g.fillText(String(corner.value), 0, 0);
        g.restore();
      }
    } else {
      const size: Record<Kind, number> = { d4: 0, d6: 0, d8: 0.34, d10: 0.3, d12: 0.33, d20: 0.27 };
      const text = String(face.value);
      let px = CELL * size[shape.kind];
      g.font = `700 ${Math.round(px)}px ${numeralFont}, Georgia, serif`;
      const maxWidth = CELL * (shape.kind === "d20" ? 0.3 : 0.36);
      const width = g.measureText(text).width;
      if (width > maxWidth) {
        px *= maxWidth / width;
        g.font = `700 ${Math.round(px)}px ${numeralFont}, Georgia, serif`;
      }
      // The kite's numeral sits in its wide half, toward the pole.
      const lift = shape.kind === "d10" ? -CELL * 0.08 : 0;
      g.fillText(text, 0, lift);
      if (shape.faces.length >= 9 && (face.value === 6 || face.value === 9)) g.fillRect(-px * 0.24, lift + px * 0.42, px * 0.48, Math.max(2, px * 0.07));
    }
    g.restore();
  });
}

interface Kit { shape: Shape; canvas: HTMLCanvasElement; texture: THREE.CanvasTexture; material: THREE.MeshStandardMaterial }

/** Geometry, numeral atlas and material per kind, shared by every die of that kind. */
export class DiceKit {
  private readonly kits = new Map<Kind, Kit>();
  private palette: Palette;

  constructor(palette: Palette) { this.palette = palette; }

  get(kind: Kind): Kit {
    let kit = this.kits.get(kind);
    if (!kit) {
      const shape = buildShape(kind);
      const canvas = document.createElement("canvas");
      drawAtlas(canvas, shape, this.palette);
      const texture = new THREE.CanvasTexture(canvas);
      texture.colorSpace = THREE.SRGBColorSpace;
      texture.anisotropy = 4;
      const material = new THREE.MeshStandardMaterial({ map: texture, roughness: 0.42, metalness: 0, flatShading: true });
      kit = { shape, canvas, texture, material };
      this.kits.set(kind, kit);
    }
    return kit;
  }

  setPalette(palette: Palette): void {
    this.palette = palette;
    for (const kit of this.kits.values()) {
      drawAtlas(kit.canvas, kit.shape, palette);
      kit.texture.needsUpdate = true;
    }
  }

  dispose(): void {
    for (const kit of this.kits.values()) {
      kit.shape.geometry.dispose();
      kit.texture.dispose();
      kit.material.dispose();
    }
    this.kits.clear();
  }
}

/** How a die rests showing `value`: its orientation and the height of its centre. */
export function restPose(shape: Shape, value: number, yawJitter: number): { quaternion: THREE.Quaternion; height: number } {
  const face = shape.faces.find((f) => f.value === value) ?? shape.faces[0]!;
  const Y = new THREE.Vector3(0, 1, 0);
  if (shape.kind === "d4") {
    // A d4 rests on the face without its number; that number stands at the top corner.
    const tilt = new THREE.Quaternion().setFromUnitVectors(face.normal, Y.clone().negate());
    const yaw = new THREE.Quaternion().setFromAxisAngle(Y, yawJitter * Math.PI);
    return { quaternion: yaw.multiply(tilt), height: face.center.dot(face.normal) };
  }
  const tilt = new THREE.Quaternion().setFromUnitVectors(face.normal, Y);
  const up = face.up.clone().applyQuaternion(tilt);
  // Turn so the numeral reads upright from the camera, with a little natural skew.
  const angle = Math.PI - Math.atan2(up.x, up.z) + yawJitter * 0.45;
  const yaw = new THREE.Quaternion().setFromAxisAngle(Y, angle);
  return { quaternion: yaw.multiply(tilt), height: face.center.dot(face.normal) };
}

export interface FloorRect { x0: number; x1: number; z0: number; z1: number }

/** The part of the felt the camera sees, inset from the frame. */
export function visibleFloor(camera: THREE.PerspectiveCamera, inset = 0.84): FloorRect {
  camera.updateMatrixWorld();
  const hit = (x: number, y: number) => {
    const ray = new THREE.Raycaster();
    ray.setFromCamera(new THREE.Vector2(x, y), camera);
    const point = new THREE.Vector3();
    return ray.ray.intersectPlane(new THREE.Plane(new THREE.Vector3(0, 1, 0), 0), point) ? point : new THREE.Vector3();
  };
  const near = hit(inset, -inset);
  const far = hit(inset, inset * 0.92);
  return { x0: -near.x, x1: near.x, z0: far.z, z1: near.z };
}

export interface Placement { x: number; z: number; scale: number }

/** Where each of `count` dice rests: a jittered grid over the floor, from the roll's seed. */
export function layout(count: number, seed: number, rect: FloorRect): Placement[] {
  if (!count) return [];
  const random = seeded(seed ^ 0x5bd1e995);
  const width = rect.x1 - rect.x0, depth = rect.z1 - rect.z0;
  let cols = Math.max(1, Math.round(Math.sqrt((count * width) / depth)));
  const rows = Math.ceil(count / cols);
  cols = Math.ceil(count / rows);
  const cellW = width / cols, cellD = depth / rows;
  const scale = Math.max(0.5, Math.min(1.75, Math.min(cellW, cellD) * 0.4));
  const roomX = Math.max(0, cellW / 2 - scale * 1.1), roomZ = Math.max(0, cellD / 2 - scale * 1.1);
  const placements: Placement[] = [];
  for (let i = 0; i < count; i++) {
    const row = Math.floor(i / cols);
    const inRow = row === rows - 1 ? count - row * cols : cols;
    const col = i % cols;
    const offset = (cols - inRow) / 2;
    placements.push({
      x: rect.x0 + (offset + col + 0.5) * cellW + (random() - 0.5) * 2 * roomX * 0.8,
      z: rect.z0 + (row + 0.5) * cellD + (random() - 0.5) * 2 * roomZ * 0.8,
      scale,
    });
  }
  return placements;
}

export interface Tumble {
  axis: THREE.Vector3;
  turns: number;
  start: THREE.Vector3;
  delay: number;
  duration: number;
  bounce: number;
}

/** A die's flight into the tray, from the roll's seed and its place in the roll. */
export function tumble(seed: number, index: number, rest: Placement, rect: FloorRect): Tumble {
  const random = seeded((seed + index * 7919) >>> 0);
  const axis = new THREE.Vector3(random() - 0.5, random() - 0.5, random() - 0.5);
  if (axis.lengthSq() < 1e-4) axis.set(1, 0, 0);
  axis.normalize();
  return {
    axis,
    turns: 2 + random() * 2.5,
    start: new THREE.Vector3(rest.x * 0.35 + (random() - 0.5) * 3, 2.6 + random() * 1.4, rect.z1 + 2.2 + random()),
    delay: index * 45 + random() * 60,
    duration: 1150 + random() * 400,
    bounce: 2.2 + random() * 1.2,
  };
}

/** Pose at `elapsed` ms. At the end it is exactly the rest pose, so the animation lands on the value. */
export function tumblePose(
  flight: Tumble, rest: Placement, resting: { quaternion: THREE.Quaternion; height: number }, reach: number, elapsed: number,
  position: THREE.Vector3, quaternion: THREE.Quaternion,
): boolean {
  const s = Math.min(1, Math.max(0, (elapsed - flight.delay) / flight.duration));
  const eased = 1 - (1 - s) ** 3;
  const remaining = 1 - eased;
  const restY = resting.height * rest.scale;
  // Hops that shrink as it settles, kept clear of the felt while it is still turning.
  const hop = flight.start.y * (1 - s) ** 2 * Math.abs(Math.cos(Math.PI * s * flight.bounce));
  const clearance = (reach - resting.height) * rest.scale * Math.min(1, remaining * 3);
  position.set(
    flight.start.x + (rest.x - flight.start.x) * eased,
    restY + clearance + hop,
    flight.start.z + (rest.z - flight.start.z) * eased,
  );
  quaternion.setFromAxisAngle(flight.axis, flight.turns * Math.PI * 2 * remaining).multiply(resting.quaternion);
  return s >= 1;
}
