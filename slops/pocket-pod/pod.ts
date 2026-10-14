import { refuse, type Tx, type Value } from "hitslop";
import type schema from "./schema";
import { repeatModes, stickerKinds } from "./schema";

export type Data = Value<typeof schema.descriptor>;
export type Video = Data["videos"][number];
export type Sticker = Data["stickers"][number];
export type StickerKind = (typeof stickerKinds)[number];
export type RepeatMode = (typeof repeatModes)[number];
type Edit = Tx<typeof schema>;
/** A uniform draw in [0, 1): `Math.random` on the page, `ctx.random` in a command. */
export type Random = () => number;

export const clamp = (value: number, low: number, high: number) => Math.max(low, Math.min(high, value));
export const round = (value: number, places: number) => Math.round(value * 10 ** places) / 10 ** places;
const idShape = /^[A-Za-z0-9_-]{11}$/;

/** What every view reads: the library, the current video and the case's name. */
export function podView(data: Data) {
  const videos = data.videos;
  const current = videos.find((video) => video.$id === data.nowPlayingId) ?? videos[0];
  const currentIndex = Math.max(0, videos.findIndex((video) => video.$id === current?.$id));
  const name = data.ownerName.trim();
  return { videos, current, currentIndex, menuTitle: name ? `${name}’s Pod` : "Pocket Pod" };
}

/**
 * The video that follows the current one. Shuffle draws another at random; otherwise the list
 * order, wrapping when stepping by hand or when repeat is "all". Nothing when the list ran out.
 */
export function nextVideo(data: Data, direction: 1 | -1, automatic: boolean, random: Random): Video | undefined {
  const { videos, currentIndex } = podView(data);
  if (!videos.length) return undefined;
  if (data.shuffle && videos.length > 1) {
    const other = (currentIndex + 1 + Math.floor(random() * (videos.length - 1))) % videos.length;
    return videos[other];
  }
  const next = currentIndex + direction;
  if (next >= 0 && next < videos.length) return videos[next];
  return !automatic || data.repeat === "all" ? videos[(next + videos.length) % videos.length] : undefined;
}

function findVideo(data: Data, id: string): Video {
  return data.videos.find((video) => video.$id === id) ?? refuse("No video with that $id.");
}
function findSticker(data: Data, id: string): Sticker {
  return data.stickers.find((sticker) => sticker.$id === id) ?? refuse("No sticker with that $id.");
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.

/** Add a looked-up video; the first one also becomes current. Returns its $id. */
export function addVideo(tx: Edit, data: Data, video: { youtubeId: string; title: string; author?: string }): string {
  const youtubeId = video.youtubeId.trim();
  if (!idShape.test(youtubeId)) refuse("A YouTube video id is 11 letters, digits, - or _.");
  if (data.videos.some((row) => row.youtubeId === youtubeId)) refuse("That video is already on your Pod.");
  const title = video.title.trim().slice(0, 200) || "Untitled video";
  const author = (video.author ?? "").trim().slice(0, 120);
  const { id } = tx.fields.videos.insert({ youtubeId, title, author });
  if (!podView(data).current) tx.fields.nowPlayingId.set(id);
  return id;
}

export function selectVideo(tx: Edit, data: Data, id: string): void {
  findVideo(data, id);
  if (data.nowPlayingId !== id) tx.fields.nowPlayingId.set(id);
}

/** Remove a video. When it was current, its neighbour takes over. */
export function removeVideo(tx: Edit, data: Data, id: string): void {
  findVideo(data, id);
  const index = data.videos.findIndex((video) => video.$id === id);
  const wasCurrent = podView(data).current?.$id === id;
  const neighbour = data.videos[index + 1] ?? data.videos[index - 1];
  tx.fields.videos.remove(id);
  if (!wasCurrent) return;
  if (neighbour) tx.fields.nowPlayingId.set(neighbour.$id);
  else tx.fields.nowPlayingId.clear();
}

/** Turn shuffle on and make a random video current. Returns its $id. */
export function shuffleAll(tx: Edit, data: Data, random: Random): string {
  if (!data.videos.length) refuse("Add a video first.");
  const video = data.videos[Math.min(data.videos.length - 1, Math.floor(random() * data.videos.length))]!;
  if (!data.shuffle) tx.fields.shuffle.set(true);
  if (data.nowPlayingId !== video.$id) tx.fields.nowPlayingId.set(video.$id);
  return video.$id;
}

export function setVolume(tx: Edit, level: number): void {
  if (!Number.isFinite(level) || level < 0 || level > 1) refuse("Volume is between 0 and 1.");
  tx.fields.volume.set(round(level, 2));
}

export function cycleRepeat(tx: Edit, data: Data): void {
  tx.fields.repeat.set(repeatModes[(repeatModes.indexOf(data.repeat) + 1) % repeatModes.length]!);
}

export function setOwnerName(tx: Edit, name: string): void {
  const trimmed = name.trim();
  if (trimmed.length > 24) refuse("Keep the name to 24 characters.");
  tx.fields.ownerName.set(trimmed);
}

/** Where a new sticker lands: the band under the screen, or beside the wheel. */
function stickerSpot(random: Random): { x: number; y: number } {
  const pick = random();
  if (pick < 0.5) return { x: round(0.12 + random() * 0.76, 3), y: 0.488 };
  return { x: pick < 0.75 ? 0.085 : 0.915, y: round(0.6 + random() * 0.25, 3) };
}

/** Stick a sticker at a random spot and tilt. Returns its $id. */
export function addSticker(tx: Edit, kind: StickerKind, random: Random): string {
  if (!stickerKinds.includes(kind)) refuse("Unknown sticker.");
  const { id } = tx.fields.stickers.insert({ kind, ...stickerSpot(random), rotation: round(-18 + random() * 36, 1) });
  return id;
}

export function placeSticker(tx: Edit, data: Data, id: string, x: number, y: number): void {
  findSticker(data, id);
  const row = tx.fields.stickers.item(id);
  row.x.set(round(clamp(x, 0, 1), 3));
  row.y.set(round(clamp(y, 0, 1), 3));
}

/** Turn by `degrees`, wrapping into [-180, 180). */
export function turnSticker(tx: Edit, data: Data, id: string, degrees: number): void {
  const sticker = findSticker(data, id);
  const next = ((((sticker.rotation + degrees + 180) % 360) + 360) % 360) - 180;
  tx.fields.stickers.item(id).rotation.set(round(next, 1));
}

export function sizeSticker(tx: Edit, data: Data, id: string, factor: number): void {
  const sticker = findSticker(data, id);
  if (!Number.isFinite(factor) || factor <= 0) refuse("Scale by a positive factor.");
  tx.fields.stickers.item(id).scale.set(round(clamp(sticker.scale * factor, 0.5, 2), 2));
}

export function peelSticker(tx: Edit, data: Data, id: string): void {
  findSticker(data, id);
  tx.fields.stickers.remove(id);
}

/** Peel every sticker. Returns how many came off. */
export function clearStickers(tx: Edit, data: Data): number {
  if (!data.stickers.length) refuse("No stickers to peel.");
  for (const sticker of data.stickers) tx.fields.stickers.remove(sticker.$id);
  return data.stickers.length;
}
