import type { SlopDocument } from "@hitslop/document/svelte";
import schema, { repeatModes, stickerKinds, type StickerKind, type Video } from "./schema";
import { click } from "./clicker";
import type { Intent } from "./intents";
import { LookupError, youtube, type YouTubeHandlers, type YouTubePlayer } from "@hitslop/document/embed";

type Doc = SlopDocument<typeof schema.descriptor>;

export type ScreenId =
  | "menu" | "coverflow" | "videos" | "add" | "playing"
  | "settings" | "stickers" | "stickerPack" | "remove";
export type Entry = { id: ScreenId; cursor: number };
export type Notice = { text: string; tone: "info" | "ok" | "error" };
export type Playback = {
  /** An embed exists for the current video. */
  started: boolean;
  /** The embed has answered at least once. */
  ready: boolean;
  /** Controls for the mounted embed, if any. */
  controller: YouTubePlayer | null;
  /** -1 unstarted, 0 ended, 1 playing, 2 paused, 3 buffering, 5 cued. */
  state: number;
  time: number;
  duration: number;
  /** A YouTube error code, or -1 when the player never answered. */
  error: number | null;
};

export const menuItems = ["Now Playing", "Cover Flow", "Videos", "Add Video", "Shuffle", "Stickers", "Settings"] as const;
export const settingsItems = ["Clicker", "Repeat", "Shuffle", "Name"] as const;
export const stickerItems = ["Add Sticker", "Remove Last", "Clear All"] as const;

const SEEK_PER_DETENT = 5;
const SEEK_PER_HOLD_TICK = 8;
const VOLUME_PER_DETENT = 0.04;
const clamp = (value: number, low: number, high: number) => Math.max(low, Math.min(high, value));
const round = (value: number, places: number) => Math.round(value * 10 ** places) / 10 ** places;
const shorten = (text: string) => (text.length > 34 ? `${text.slice(0, 33)}…` : text);

/** Where a new sticker lands: the band under the screen, or beside the wheel. */
function stickerSpot(): { x: number; y: number } {
  const pick = Math.random();
  if (pick < 0.5) return { x: round(0.12 + Math.random() * 0.76, 3), y: 0.488 };
  return { x: pick < 0.75 ? 0.085 : 0.915, y: round(0.6 + Math.random() * 0.25, 3) };
}

export function createPod(doc: Doc) {
  const ui = $state({
    stack: [{ id: "menu", cursor: 0 }] as Entry[],
    held: false,
    scrub: false,
    renaming: false,
    addText: "",
    busy: false,
    notice: null as Notice | null,
    volume: null as number | null,
    volumeVisible: false,
    removeId: null as string | null,
    selectedSticker: null as string | null,
  });
  const playback = $state<Playback>({
    started: false, ready: false, controller: null, state: -1, time: 0, duration: 0, error: null,
  });

  const videos = $derived(doc.current.videos);
  const current = $derived(videos.find((video) => video.$id === doc.current.nowPlayingId) ?? videos[0]);
  const top = $derived(ui.stack[ui.stack.length - 1]!);
  const menuTitle = $derived(doc.current.ownerName.trim() ? `${doc.current.ownerName.trim()}’s Pod` : "Pocket Pod");

  let noticeTimer: ReturnType<typeof setTimeout> | undefined;
  let volumeTimer: ReturnType<typeof setTimeout> | undefined;
  let volumeHideTimer: ReturnType<typeof setTimeout> | undefined;

  function flash(text: string, tone: Notice["tone"] = "info"): void {
    ui.notice = { text, tone };
    clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => { ui.notice = null; }, 2800);
  }
  const tick = () => { if (doc.current.clicker) click(); };
  const indexOfCurrent = () => Math.max(0, videos.findIndex((video) => video.$id === current?.$id));

  function push(id: ScreenId, cursor = 0): void {
    ui.stack.push({ id, cursor });
    ui.scrub = false;
    ui.renaming = false;
  }
  function pop(): void {
    if (ui.stack.length < 2) return;
    const left = ui.stack.pop()!;
    ui.scrub = false;
    ui.renaming = false;
    if (left.id === "stickers" || left.id === "stickerPack") ui.selectedSticker = null;
  }
  function setCursor(index: number): void {
    ui.stack[ui.stack.length - 1]!.cursor = index;
  }

  // Library -------------------------------------------------------------------------------

  function pick(direction: 1 | -1, automatic: boolean): Video | undefined {
    if (!videos.length) return undefined;
    const index = indexOfCurrent();
    if (doc.current.shuffle && videos.length > 1) {
      let other = index;
      while (other === index) other = Math.floor(Math.random() * videos.length);
      return videos[other];
    }
    const next = index + direction;
    if (next >= 0 && next < videos.length) return videos[next];
    return !automatic || doc.current.repeat === "all" ? videos[(next + videos.length) % videos.length] : undefined;
  }

  async function play(video: Video): Promise<void> {
    playback.started = true;
    playback.ready = false;
    playback.state = -1;
    playback.time = 0;
    playback.duration = 0;
    playback.error = null;
    if (doc.current.nowPlayingId !== video.$id) await doc.fields.nowPlayingId.set(video.$id);
  }

  function resume(): void {
    if (playback.state !== 1) playback.controller?.play();
  }

  function playPause(): void {
    if (!current) { flash("Paste a YouTube link to add a video."); return; }
    const controller = playback.controller;
    if (!playback.started || !controller) { void play(current); return; }
    if (playback.state === 1 || playback.state === 3) controller.pause();
    else controller.play();
  }

  function skip(direction: 1 | -1): void {
    if (!videos.length) return;
    if (direction === -1 && playback.started && playback.time > 3 && playback.controller) {
      playback.controller.seek(0);
      playback.time = 0;
      return;
    }
    const target = pick(direction, false);
    if (!target) return;
    if (playback.started) void play(target);
    else void doc.fields.nowPlayingId.set(target.$id);
  }

  function seekBy(seconds: number): void {
    const controller = playback.controller;
    if (!playback.started || !controller) return;
    const next = clamp(playback.time + seconds, 0, playback.duration || Number.MAX_SAFE_INTEGER);
    playback.time = next;
    controller.seek(next);
  }

  function nudgeVolume(delta: number): void {
    const level = round(clamp((ui.volume ?? doc.current.volume) + delta, 0, 1), 2);
    ui.volume = level;
    ui.volumeVisible = true;
    playback.controller?.setVolume(level);
    clearTimeout(volumeTimer);
    volumeTimer = setTimeout(() => {
      ui.volume = null;
      void doc.fields.volume.set(level);
    }, 400);
    clearTimeout(volumeHideTimer);
    volumeHideTimer = setTimeout(() => { ui.volumeVisible = false; }, 1600);
  }

  async function ended(): Promise<void> {
    if (doc.current.repeat === "one" && playback.controller) {
      playback.controller.seek(0);
      playback.controller.play();
      return;
    }
    const next = pick(1, true);
    if (next) await play(next);
    else playback.started = false;
  }

  const handlers: YouTubeHandlers = {
    onReady() {
      playback.ready = true;
      playback.controller?.setVolume(doc.current.volume);
    },
    onState(state) {
      playback.state = state;
      if (state === 1) playback.error = null;
    },
    onTime(time, duration) {
      playback.time = time;
      if (duration !== undefined) playback.duration = duration;
    },
    onEnded: () => { void ended(); },
    onError(code) { playback.error = code; },
  };
  function attach(controller: YouTubePlayer | null): void {
    playback.controller = controller;
  }

  function focusRow(id: string): void {
    const index = doc.current.videos.findIndex((video) => video.$id === id);
    if (index >= 0 && (top.id === "coverflow" || top.id === "videos")) setCursor(index);
  }

  async function addFromText(text: string): Promise<boolean> {
    const youtubeId = youtube.findId(text);
    if (!youtubeId) { flash("That doesn’t look like a YouTube link.", "error"); return false; }
    const existing = doc.current.videos.find((video) => video.youtubeId === youtubeId);
    if (existing) { focusRow(existing.$id); flash("That video is already on your Pod."); return true; }
    if (ui.busy) return false;
    ui.busy = true;
    try {
      const meta = await youtube.lookup(youtubeId);
      if (doc.current.videos.some((video) => video.youtubeId === youtubeId)) return true;
      const { id } = await doc.fields.videos.insert(meta);
      if (!doc.current.nowPlayingId) await doc.fields.nowPlayingId.set(id);
      focusRow(id);
      flash(`Added “${shorten(meta.title)}”`, "ok");
      return true;
    } catch (cause) {
      flash(cause instanceof LookupError ? cause.message : "Couldn’t add that video.", "error");
      return false;
    } finally {
      ui.busy = false;
    }
  }

  async function submitAdd(): Promise<void> {
    const text = ui.addText.trim();
    if (!text) { flash("Paste a YouTube link first."); return; }
    if (await addFromText(text)) ui.addText = "";
  }

  async function removeVideo(id: string): Promise<void> {
    const list = doc.current.videos;
    const index = list.findIndex((video) => video.$id === id);
    if (index < 0) return;
    const wasCurrent = current?.$id === id;
    const neighbour = list[index + 1] ?? list[index - 1];
    await doc.change((tx) => {
      tx.fields.videos.remove(id);
      if (wasCurrent) {
        if (neighbour) tx.fields.nowPlayingId.set(neighbour.$id);
        else tx.fields.nowPlayingId.clear();
      }
    });
    if (wasCurrent) playback.started = false;
    for (const entry of ui.stack) {
      if (entry.id === "coverflow" || entry.id === "videos") entry.cursor = clamp(entry.cursor, 0, Math.max(0, list.length - 2));
    }
  }

  // Stickers ------------------------------------------------------------------------------

  async function addSticker(kind: StickerKind): Promise<void> {
    const { id } = await doc.fields.stickers.insert({
      kind, ...stickerSpot(), rotation: round(-18 + Math.random() * 36, 1), scale: 1,
    });
    ui.selectedSticker = id;
  }
  async function peelSticker(id: string): Promise<void> {
    if (ui.selectedSticker === id) ui.selectedSticker = null;
    await doc.fields.stickers.remove(id);
  }
  async function placeSticker(id: string, x: number, y: number): Promise<void> {
    await doc.change((tx) => {
      const handle = tx.fields.stickers.item(id);
      handle.x.set(round(clamp(x, 0, 1), 3));
      handle.y.set(round(clamp(y, 0, 1), 3));
    });
  }
  async function turnSticker(id: string, degrees: number): Promise<void> {
    const sticker = doc.current.stickers.find((item) => item.$id === id);
    if (!sticker) return;
    const next = ((((sticker.rotation + degrees + 180) % 360) + 360) % 360) - 180;
    await doc.fields.stickers.item(id).rotation.set(round(next, 1));
  }
  async function sizeSticker(id: string, factor: number): Promise<void> {
    const sticker = doc.current.stickers.find((item) => item.$id === id);
    if (!sticker) return;
    await doc.fields.stickers.item(id).scale.set(round(clamp(sticker.scale * factor, 0.5, 2), 2));
  }
  async function removeLastSticker(): Promise<void> {
    const last = doc.current.stickers.at(-1);
    if (last) await peelSticker(last.$id);
    else flash("No stickers to peel.");
  }
  async function clearStickers(): Promise<void> {
    const all = doc.current.stickers;
    if (!all.length) { flash("No stickers to peel."); return; }
    ui.selectedSticker = null;
    await doc.change((tx) => { for (const sticker of all) tx.fields.stickers.remove(sticker.$id); });
  }

  // Wheel intents -------------------------------------------------------------------------

  function scroll(delta: 1 | -1): void {
    const entry = top;
    const within = (max: number) => clamp(entry.cursor + delta, 0, Math.max(0, max));
    let next = entry.cursor;
    switch (entry.id) {
      case "menu": next = within(menuItems.length - 1); break;
      case "coverflow": case "videos": next = within(videos.length - 1); break;
      case "settings": if (ui.renaming) return; next = within(settingsItems.length - 1); break;
      case "stickers": next = within(stickerItems.length - 1); break;
      case "stickerPack": next = within(stickerKinds.length - 1); break;
      case "playing":
        if (ui.scrub) seekBy(delta * SEEK_PER_DETENT);
        else nudgeVolume(delta * VOLUME_PER_DETENT);
        tick();
        return;
      default: return;
    }
    if (next !== entry.cursor) { setCursor(next); tick(); }
  }

  function selectMenu(index: number): void {
    switch (index) {
      case 0:
        if (!current) { flash("Paste a YouTube link to add a video."); push("add"); return; }
        if (!playback.started) void play(current);
        push("playing");
        return;
      case 1: push("coverflow", indexOfCurrent()); return;
      case 2: push("videos", indexOfCurrent()); return;
      case 3: push("add"); return;
      case 4: {
        if (!videos.length) { flash("Add a video first."); return; }
        void doc.fields.shuffle.set(true);
        void play(videos[Math.floor(Math.random() * videos.length)]!);
        push("playing");
        return;
      }
      case 5: push("stickers"); return;
      case 6: push("settings"); return;
    }
  }

  function selectSetting(index: number): void {
    if (index === 0) void doc.fields.clicker.set(!doc.current.clicker);
    else if (index === 1) void doc.fields.repeat.set(repeatModes[(repeatModes.indexOf(doc.current.repeat) + 1) % repeatModes.length]!);
    else if (index === 2) void doc.fields.shuffle.set(!doc.current.shuffle);
    else ui.renaming = !ui.renaming;
  }

  function select(): void {
    switch (top.id) {
      case "menu": selectMenu(top.cursor); return;
      case "coverflow": case "videos": {
        const video = videos[top.cursor];
        if (!video) return;
        if (playback.started && current?.$id === video.$id) resume();
        else void play(video);
        push("playing");
        return;
      }
      case "playing":
        if (playback.started) ui.scrub = !ui.scrub;
        else playPause();
        return;
      case "add": void submitAdd(); return;
      case "settings": selectSetting(top.cursor); return;
      case "stickers":
        if (top.cursor === 0) push("stickerPack");
        else if (top.cursor === 1) void removeLastSticker();
        else void clearStickers();
        return;
      case "stickerPack": {
        const kind = stickerKinds[top.cursor];
        if (kind) void addSticker(kind);
        return;
      }
      case "remove":
        if (ui.removeId) void removeVideo(ui.removeId);
        pop();
        return;
    }
  }

  function menu(): void {
    if (ui.renaming) ui.renaming = false;
    else if (ui.stack.length > 1) pop();
    else ui.selectedSticker = null;
  }

  function holdSelect(): void {
    if (top.id !== "coverflow" && top.id !== "videos") return;
    const video = videos[top.cursor];
    if (!video) return;
    ui.removeId = video.$id;
    push("remove");
  }

  function dispatch(intent: Intent): void {
    if (ui.held) return;
    switch (intent.type) {
      case "scroll": scroll(intent.delta); break;
      case "select": select(); break;
      case "hold-select": holdSelect(); break;
      case "menu": menu(); break;
      case "playpause": playPause(); break;
      case "next": skip(1); break;
      case "prev": skip(-1); break;
      case "seek": seekBy(intent.delta * SEEK_PER_HOLD_TICK); break;
    }
  }

  function dispose(): void {
    clearTimeout(noticeTimer);
    clearTimeout(volumeTimer);
    clearTimeout(volumeHideTimer);
  }

  return {
    ui, playback, dispatch, dispose, flash, setCursor, addFromText,
    handlers, attach, addSticker, peelSticker, placeSticker, turnSticker, sizeSticker,
    get videos() { return videos; },
    get current() { return current; },
    get top() { return top; },
    get menuTitle() { return menuTitle; },
  };
}

export type PodController = ReturnType<typeof createPod>;
