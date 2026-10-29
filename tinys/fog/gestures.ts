// Pure gesture, breath and mouth logic for the live mirror. No DOM, clock or randomness here:
// the view passes points in device pixels, times in milliseconds and a random source.
// The numbers are the brief's tuned values; change them only by hand-held testing.

export type Point = { x: number; y: number };

// MediaPipe hand landmark indices (the 21-point model; HandLandmarker returns the same set).
export const WRIST = 0, THUMB_TIP = 4, INDEX_MCP = 5, INDEX_PIP = 6, INDEX_TIP = 8,
  MIDDLE_MCP = 9, MIDDLE_PIP = 10, MIDDLE_TIP = 12, RING_PIP = 14, RING_TIP = 16,
  PINKY_MCP = 17, PINKY_PIP = 18, PINKY_TIP = 20;

export const TUNING = {
  fogOnLoad: 0.55,
  pinchDown: 0.22,
  pinchUp: 0.38,
  breathTrigger: 0.0025,
  breathFull: 0.003,
  floorDown: 0.05,
  floorUp: 0.0008,
  keyQuietMs: 300,
  breathBlobs: 12,
  breathSpreadX: 1.45,
  breathBlur: 0.6,
  breathReach: 0.43,
  breathAlpha: 0.36,
  centreEase: 0.4,
  rawToTarget: 0.45,
  brushChase: 0.22,
  starChase: 0.55,
  minMove: 0.0006,
  vanishMs: 250,
  releaseFrames: 2,
  wipeScale: 1.6,
  wipeAlpha: 0.34,
  wipeMaxStamps: 40,
  wipeJump: 0.3,
  dprCap: 2,
  // Trackers: one hand; detection, presence and tracking confidence.
  handDetection: 0.6,
  handPresence: 0.5,
  handTracking: 0.5,
  faceDetection: 0.5,
  // The face is looked for on every 5th tracked frame; a face missing for 1 s falls back
  // to the fixed mouth (centred, 62% down the camera frame).
  faceEvery: 5,
  faceLostMs: 1000,
  mouthFallbackX: 0.5,
  mouthFallbackY: 0.62,
  // Without a mouth keypoint: 0.22 of the face box below the nose, else 0.78 down the box.
  mouthBelowNose: 0.22,
  mouthInBox: 0.78,
  // The breath centres this far (of the short edge) below the mouth.
  breathBelowMouth: 0.04,
} as const;

export const distance = (a: Point, b: Point) => Math.hypot(a.x - b.x, a.y - b.y);
export const ease = (from: Point, to: Point, k: number): Point => ({ x: from.x + (to.x - from.x) * k, y: from.y + (to.y - from.y) * k });
export const midpoint = (a: Point, b: Point): Point => ({ x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 });

/** Every pixel size is multiplied by this: the screen's density, capped at 2. */
export const pixelRatio = (devicePixelRatio: number) => Math.min(TUNING.dprCap, Math.max(1, devicePixelRatio || 1));

/** Thumb tip to index tip, relative to the hand's size (wrist to middle knuckle). */
export function pinchRatio(hand: readonly Point[]): number {
  const size = distance(hand[WRIST]!, hand[MIDDLE_MCP]!);
  return size > 0 ? distance(hand[THUMB_TIP]!, hand[INDEX_TIP]!) / size : Infinity;
}

/** Pen down below 0.22, up above 0.38, unchanged in between (hysteresis). */
export function pinchPen(down: boolean, ratio: number): boolean {
  if (ratio < TUNING.pinchDown) return true;
  if (ratio > TUNING.pinchUp) return false;
  return down;
}

/** A fist: all four fingertips are closer to the wrist than their middle joints. */
export function isFist(hand: readonly Point[]): boolean {
  const wrist = hand[WRIST]!;
  return ([[INDEX_TIP, INDEX_PIP], [MIDDLE_TIP, MIDDLE_PIP], [RING_TIP, RING_PIP], [PINKY_TIP, PINKY_PIP]] as const)
    .every(([tip, pip]) => distance(hand[tip]!, wrist) < distance(hand[pip]!, wrist));
}

/** Drawing needs the index finger out, checked only when the pen goes down. */
export function indexExtended(hand: readonly Point[]): boolean {
  const wrist = hand[WRIST]!;
  return distance(hand[INDEX_TIP]!, wrist) > distance(hand[INDEX_PIP]!, wrist);
}

/** The larger of index-to-pinky knuckle and wrist-to-middle knuckle. */
export function handWidth(hand: readonly Point[]): number {
  return Math.max(distance(hand[INDEX_MCP]!, hand[PINKY_MCP]!), distance(hand[WRIST]!, hand[MIDDLE_MCP]!));
}

export const pinchPoint = (hand: readonly Point[]) => midpoint(hand[THUMB_TIP]!, hand[INDEX_TIP]!);
export const palmCentre = (hand: readonly Point[]): Point => {
  const ids = [WRIST, INDEX_MCP, MIDDLE_MCP, PINKY_MCP];
  return { x: ids.reduce((s, i) => s + hand[i]!.x, 0) / ids.length, y: ids.reduce((s, i) => s + hand[i]!.y, 0) / ids.length };
};

/** The pen while the hand comes and goes: a vanished hand keeps the pen down for 250 ms,
 * and a release must hold for 2 frames before the stroke ends. */
export type PenState = { down: boolean; pinched: boolean; released: number; seenAt: number };
export const penIdle: PenState = { down: false, pinched: false, released: 0, seenAt: -Infinity };
export function stepPen(state: PenState, hand: readonly Point[] | undefined, now: number): PenState {
  if (!hand) {
    if (state.down && now - state.seenAt <= TUNING.vanishMs) return state;
    return { ...state, down: false, pinched: false, released: 0 };
  }
  const pinched = pinchPen(state.pinched, pinchRatio(hand));
  if (pinched) {
    // A new stroke needs the index finger out; an ongoing one does not re-check it.
    const down = state.down || indexExtended(hand);
    return { down, pinched, released: 0, seenAt: now };
  }
  const released = state.released + 1;
  return { down: state.down && released < TUNING.releaseFrames, pinched, released, seenAt: now };
}

/** The breath detector's floating noise floor. Returns the breath strength (0 to 1)
 * for this level against the floor as it was, and the floor after this frame. */
export type Breath = { floor: number; strength: number };
export function stepBreath(floor: number | undefined, level: number): Breath {
  if (floor === undefined) return { floor: level, strength: 0 };
  const over = level - floor;
  const strength = over > TUNING.breathTrigger
    ? Math.min(1, (over - TUNING.breathTrigger) / (TUNING.breathFull - TUNING.breathTrigger))
    : 0;
  const next = level < floor ? floor + (level - floor) * TUNING.floorDown : floor + (level - floor) * TUNING.floorUp;
  return { floor: next, strength };
}

/** Root mean square of one block of time-domain samples. */
export function rms(samples: ArrayLike<number>): number {
  let sum = 0;
  for (let i = 0; i < samples.length; i++) sum += samples[i]! * samples[i]!;
  return samples.length ? Math.sqrt(sum / samples.length) : 0;
}

/** Breath ignores 300 ms after any keystroke, so typing never fogs the glass. */
export const keyQuiet = (now: number, lastKeyAt: number) => now - lastKeyAt >= TUNING.keyQuietMs;

/** Where the fist wipe stamps, from the last stamp to the hand now: one soft stamp every
 * half radius of distance travelled, at most 40 per frame; a jump over 30% of the screen
 * skips interpolation and stamps once. A still fist stamps nothing more. Pass the last
 * stamp (not last frame's point) as `previous`, so slow movement still adds up. */
export function wipeStamps(previous: Point | undefined, current: Point, radius: number, screen: number): Point[] {
  if (!previous) return [current];
  const travelled = distance(previous, current);
  if (travelled > TUNING.wipeJump * screen) return [current];
  const step = radius / 2;
  if (!(step > 0) || travelled < step) return [];
  const steps = Math.floor(travelled / step);
  const count = Math.min(TUNING.wipeMaxStamps, steps);
  // Stamps sit every half radius along the path; when capped, they spread over it.
  const span = count === steps ? (steps * step) / travelled : 1;
  return Array.from({ length: count }, (_, i) => ease(previous, current, (span * (i + 1)) / count));
}

/** A face as the face detector reports it: a box in video pixels and keypoints normalized
 * to the frame. BlazeFace's six keypoints are right eye, left eye, nose tip, mouth centre,
 * right ear and left ear. */
export type Face = {
  box?: { originX: number; originY: number; width: number; height: number };
  keypoints: readonly Point[];
};
export const NOSE_TIP = 2, MOUTH_CENTRE = 3;

/** The mouth, normalized to the unmirrored camera frame, of the largest face: the mouth
 * keypoint when there is one, else below the nose, else low in the box. Null without a face. */
export function mouthOf(faces: readonly Face[], videoWidth: number, videoHeight: number): Point | null {
  const area = (f: Face) => (f.box ? f.box.width * f.box.height : 0);
  const face = faces.reduce<Face | undefined>((best, f) => (!best || area(f) > area(best) ? f : best), undefined);
  if (!face) return null;
  const mouth = face.keypoints[MOUTH_CENTRE];
  if (mouth) return { x: mouth.x, y: mouth.y };
  const box = face.box && videoWidth > 0 && videoHeight > 0
    ? { x: face.box.originX / videoWidth, y: face.box.originY / videoHeight, w: face.box.width / videoWidth, h: face.box.height / videoHeight }
    : undefined;
  const nose = face.keypoints[NOSE_TIP];
  if (nose) return { x: nose.x, y: nose.y + (box ? TUNING.mouthBelowNose * box.h : 0) };
  return box ? { x: box.x + box.w / 2, y: box.y + TUNING.mouthInBox * box.h } : null;
}

/** Where the breath aims, normalized: the last mouth seen, or the fixed mirror-typical
 * mouth once no face has been found for `faceLostMs`. */
export function mouthTarget(lastMouth: Point | null, lastSeenAt: number, now: number): Point {
  return lastMouth && now - lastSeenAt <= TUNING.faceLostMs ? lastMouth : { x: TUNING.mouthFallbackX, y: TUNING.mouthFallbackY };
}

/** Twelve soft blobs around a centre slightly below the mouth, 1.45 times wider than
 * tall, reaching 0.43 of the screen's short edge. `random` returns [0, 1). */
export type Blob = { x: number; y: number; r: number; alpha: number };
export function breathBlobs(centre: Point, shortEdge: number, strength: number, random: () => number): Blob[] {
  const reach = TUNING.breathReach * shortEdge;
  return Array.from({ length: TUNING.breathBlobs }, () => {
    const angle = random() * Math.PI * 2;
    const spread = Math.sqrt(random()) * reach * 0.62;
    const r = reach * (0.28 + random() * 0.3);
    return {
      x: centre.x + Math.cos(angle) * spread * TUNING.breathSpreadX,
      y: centre.y + Math.sin(angle) * spread,
      r,
      alpha: TUNING.breathAlpha * strength,
    };
  });
}
