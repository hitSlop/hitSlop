// The page's player: screen stack, playback and notices are view state that lives as long as
// the page. Every document change goes through an edit in pod.ts.
import schema, { stickerKinds } from "./schema";
import { click } from "./clicker";
import type { Intent } from "./intents";
import {
  addSticker as stickOn, addVideo, clamp, clearStickers as peelAll, cycleRepeat, nextVideo, peelSticker as peelOff,
  placeSticker as place, podView, removeVideo as removeRow, round, selectVideo, setVolume, shuffleAll,
  sizeSticker as size, turnSticker as turn, type StickerKind, type Video,
} from "./pod";
import { attachments, getDocument, getEnvironment, haptics } from "@hitslop/pop/svelte";
import { refuse } from "@hitslop/pop";
import { LookupError, youtube, type YouTubeHandlers, type YouTubePlayer } from "@hitslop/pop/embed";

export type ScreenId =
  | "menu" | "coverflow" | "videos" | "add" | "playing"
  | "settings" | "customize" | "stickers" | "stickerPack" | "remove" | "games" | "breakout";
export type Entry = { id: ScreenId; cursor: number };
export type Notice = { text: string; tone: "info" | "ok" | "error" };
export type Playback = {
  /** An embed exists for the current video. */
  started: boolean;
  /** The embed has answered at least once. */
  ready: boolean;
  /** Controls for the mounted embed, if any. */
  controller: PlaybackController | null;
  /** -1 unstarted, 0 ended, 1 playing, 2 paused, 3 buffering, 5 cued. */
  state: number;
  time: number;
  duration: number;
  /** A YouTube error code, or -1 when the player never answered. */
  error: number | null;
};
export type PlaybackController = Pick<YouTubePlayer, "play" | "pause" | "seek" | "setVolume">;

export const menuItems = ["Now Playing", "Cover Flow", "Songs", "Add Music", "Shuffle", "Customize", "Games", "Settings"] as const;
export const settingsItems = ["Clicker", "Repeat", "Shuffle", "Name"] as const;
export const stickerItems = ["Add Sticker", "Remove Last", "Clear All"] as const;

const SEEK_PER_DETENT = 5;
const SEEK_PER_HOLD_TICK = 8;
const VOLUME_PER_DETENT = 0.04;
const shorten = (text: string) => (text.length > 34 ? `${text.slice(0, 33)}…` : text);
// The page's random source: shuffle picks and sticker spots are stored once drawn.
const random = () => Math.random();

// App calls this while it initializes, when getDocument can reach the mounted document.
export function createPod() {
  const doc = getDocument(schema);
  const environment = getEnvironment();
  const systemVolume = $derived(environment.current.platform === "ios");
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

  const view = $derived(podView(doc.current));
  const videos = $derived(view.videos);
  const current = $derived(view.current);
  const top = $derived(ui.stack[ui.stack.length - 1]!);

  let noticeTimer: ReturnType<typeof setTimeout> | undefined;
  let volumeTimer: ReturnType<typeof setTimeout> | undefined;
  let volumeHideTimer: ReturnType<typeof setTimeout> | undefined;
  let game: ((intent: Intent) => void) | null = null;
  function attachGame(handler: ((intent: Intent) => void) | null) { game = handler; }

  function flash(text: string, tone: Notice["tone"] = "info"): void {
    ui.notice = { text, tone };
    clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => { ui.notice = null; }, 2800);
  }
  const tick = () => { if (doc.current.clicker) click(); haptics.selection(); };

  function push(id: ScreenId, cursor = 0): void {
    if (top.id === "breakout") game?.({ type: "menu" });
    ui.stack.push({ id, cursor });
    ui.scrub = false;
    ui.renaming = false;
  }
  function pop(): void {
    if (ui.stack.length < 2) return;
    if (top.id === "breakout") game?.({ type: "menu" });
    const left = ui.stack.pop()!;
    ui.scrub = false;
    ui.renaming = false;
    if (left.id === "stickers" || left.id === "stickerPack") ui.selectedSticker = null;
  }
  function setCursor(index: number): void {
    ui.stack[ui.stack.length - 1]!.cursor = index;
  }

  // Library -------------------------------------------------------------------------------

  function startPlayback(): void {
    playback.started = true;
    playback.ready = false;
    playback.state = -1;
    playback.time = 0;
    playback.duration = 0;
    playback.error = null;
  }

  async function play(video: Video): Promise<void> {
    if (playback.started && current?.$id === video.$id && playback.controller) {
      playback.controller.seek(0);
      playback.controller.play();
      return;
    }
    startPlayback();
    if (doc.current.nowPlayingId !== video.$id) await doc.change((tx) => selectVideo(tx, doc.current, video.$id));
  }

  function resume(): void {
    if (playback.state !== 1) playback.controller?.play();
  }

  function playPause(): void {
    if (!current) { flash("Add local songs or a YouTube link first."); return; }
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
    const target = nextVideo(doc.current, direction, false, random);
    if (!target) return;
    if (playback.started) void play(target);
    else void doc.change((tx) => selectVideo(tx, doc.current, target.$id));
  }

  function seekBy(seconds: number): void {
    const controller = playback.controller;
    if (!playback.started || !controller) return;
    const next = clamp(playback.time + seconds, 0, playback.duration || Number.MAX_SAFE_INTEGER);
    playback.time = next;
    controller.seek(next);
  }

  function nudgeVolume(delta: number): void {
    ui.volumeVisible = true;
    clearTimeout(volumeHideTimer);
    volumeHideTimer = setTimeout(() => { ui.volumeVisible = false; }, systemVolume ? 3200 : 1600);
    // iOS owns media volume. Do not change a stored level or show a working
    // meter for an adjustment its media player will silently ignore.
    if (systemVolume) return;
    const level = round(clamp((ui.volume ?? doc.current.volume) + delta, 0, 1), 2);
    ui.volume = level;
    doc.fields.volume.preview(level);
    playback.controller?.setVolume(level);
    clearTimeout(volumeTimer);
    volumeTimer = setTimeout(() => {
      ui.volume = null;
      void doc.change((tx) => setVolume(tx, level));
    }, 400);
  }

  async function ended(): Promise<void> {
    if (doc.current.repeat === "one" && playback.controller) {
      playback.controller.seek(0);
      playback.controller.play();
      return;
    }
    const next = nextVideo(doc.current, 1, true, random);
    if (next) await play(next);
    else playback.started = false;
  }

  const handlers: YouTubeHandlers = {
    onReady() {
      playback.ready = true;
      if (playback.error === -1) playback.error = null;
      if (!systemVolume) playback.controller?.setVolume(doc.current.volume);
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
  function attach(controller: PlaybackController | null): void {
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
    let meta;
    try {
      meta = await youtube.lookup(youtubeId);
    } catch (cause) {
      flash(cause instanceof LookupError ? cause.message : "Couldn’t add that video.", "error");
      return false;
    } finally {
      ui.busy = false;
    }
    // The lookup took a moment; the same link may have arrived meanwhile.
    if (doc.current.videos.some((video) => video.youtubeId === youtubeId)) return true;
    const found = meta;
    const id = await doc.change((tx) => addVideo(tx, doc.current, found));
    focusRow(id);
    flash(`Added “${shorten(found.title)}”`, "ok");
    return true;
  }

  async function submitAdd(): Promise<void> {
    const text = ui.addText.trim();
    if (!text) { flash("Paste a YouTube link first."); return; }
    if (await addFromText(text)) ui.addText = "";
  }

  async function importSongs(files: File[]): Promise<void> {
    if (ui.busy || !files.length) return;
    ui.busy = true;
    let added = 0;
    let skipped = 0;
    let failed = 0;
    let lastFailure = "";
    try {
      for (const file of files) {
        if (!file.size || file.size > 10 * 1024 * 1024 ||
          !(/\.(mp3|m4a|wav|flac|ogg|opus)$/i.test(file.name) || file.type.startsWith("audio/"))) {
          skipped++;
          continue;
        }
        const name = file.name.replace(/\.[^.]+$/, "");
        const parts = name.split(" - ");
        const author = parts.length > 1 ? parts.shift()! : "Local audio";
        const title = parts.join(" - ") || name;
        try {
          await attachments.import<typeof schema.descriptor>(file, (tx, ref) => {
            if (!ref.mimeType.startsWith("audio/")) refuse(`“${file.name}” is not a supported audio file.`);
            const { id } = tx.fields.videos.insert({
              title: Array.from(title).slice(0, 200).join(""),
              author: Array.from(author).slice(0, 120).join(""),
              audio: { id: ref.id, name: ref.name, mimeType: ref.mimeType },
            });
            if (!doc.current.videos.length) tx.fields.nowPlayingId.set(id);
          });
          added++;
        } catch (cause) {
          failed++;
          lastFailure = `${file.name}: ${cause instanceof Error ? cause.message : String(cause)}`;
        }
      }
      flash(`${added} ${added === 1 ? "song" : "songs"} added.${skipped ? ` ${skipped} skipped; use audio files up to 10 MB each.` : ""}${failed ? ` ${failed} failed. ${lastFailure}` : ""}`, added && !failed ? "ok" : "error");
      if (added) haptics.notification(failed || skipped ? "warning" : "success");
      else if (failed) haptics.notification("error");
    } finally { ui.busy = false; }
  }

  async function removeVideo(id: string): Promise<void> {
    const before = doc.current;
    if (!before.videos.some((video) => video.$id === id)) return;
    const wasCurrent = current?.$id === id;
    await doc.change((tx) => removeRow(tx, before, id));
    if (wasCurrent) playback.started = false;
    for (const entry of ui.stack) {
      if (entry.id === "coverflow" || entry.id === "videos") entry.cursor = clamp(entry.cursor, 0, Math.max(0, before.videos.length - 2));
    }
  }

  // Stickers ------------------------------------------------------------------------------

  async function addSticker(kind: StickerKind): Promise<void> {
    ui.selectedSticker = await doc.change((tx) => stickOn(tx, kind, random));
  }
  async function peelSticker(id: string): Promise<void> {
    if (ui.selectedSticker === id) ui.selectedSticker = null;
    if (!doc.current.stickers.some((sticker) => sticker.$id === id)) return;
    await doc.change((tx) => peelOff(tx, doc.current, id));
  }
  async function placeSticker(id: string, x: number, y: number): Promise<void> {
    if (!doc.current.stickers.some((sticker) => sticker.$id === id)) return;
    await doc.change((tx) => place(tx, doc.current, id, x, y));
  }
  async function turnSticker(id: string, degrees: number): Promise<void> {
    if (!doc.current.stickers.some((sticker) => sticker.$id === id)) return;
    await doc.change((tx) => turn(tx, doc.current, id, degrees));
  }
  async function sizeSticker(id: string, factor: number): Promise<void> {
    if (!doc.current.stickers.some((sticker) => sticker.$id === id)) return;
    await doc.change((tx) => size(tx, doc.current, id, factor));
  }
  async function removeLastSticker(): Promise<void> {
    const last = doc.current.stickers.at(-1);
    if (last) await peelSticker(last.$id);
    else flash("No stickers to peel.");
  }
  async function clearStickers(): Promise<void> {
    if (!doc.current.stickers.length) { flash("No stickers to peel."); return; }
    ui.selectedSticker = null;
    await doc.change((tx) => peelAll(tx, doc.current));
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
      case "games": return;
      case "playing":
        if (ui.scrub) seekBy(delta * SEEK_PER_DETENT);
        else nudgeVolume(delta * VOLUME_PER_DETENT);
        tick();
        return;
      default: return;
    }
    if (next !== entry.cursor) { setCursor(next); tick(); }
  }

  async function shuffleAndPlay(): Promise<void> {
    if (!videos.length) { flash("Add a song first."); return; }
    startPlayback();
    push("playing");
    await doc.change((tx) => shuffleAll(tx, doc.current, random));
  }

  function selectMenu(index: number): void {
    switch (index) {
      case 0:
        if (!current) { flash("Add local songs or a YouTube link first."); push("add"); return; }
        if (!playback.started) void play(current);
        push("playing");
        return;
      case 1: push("coverflow", view.currentIndex); return;
      case 2: push("videos", view.currentIndex); return;
      case 3: push("add"); return;
      case 4: void shuffleAndPlay(); return;
      case 5: push("customize"); return;
      case 6: push("games"); return;
      case 7: push("settings"); return;
    }
  }

  function selectSetting(index: number): void {
    if (index === 0) void doc.change((tx) => tx.fields.clicker.set(!doc.current.clicker));
    else if (index === 1) void doc.change((tx) => cycleRepeat(tx, doc.current));
    else if (index === 2) void doc.change((tx) => tx.fields.shuffle.set(!doc.current.shuffle));
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
        if (playback.started) { ui.scrub = !ui.scrub; ui.volumeVisible = false; }
        else playPause();
        return;
      case "add": void submitAdd(); return;
      case "games": push("breakout"); return;
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
    if (top.id === "breakout" && ["scroll", "prev", "next", "select", "hold-select", "seek"].includes(intent.type)) {
      game?.(intent);
      return;
    }
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
    ui, playback, dispatch, dispose, flash, setCursor, addFromText, submitAdd, importSongs, push,
    handlers, attach, attachGame, addSticker, peelSticker, placeSticker, turnSticker, sizeSticker,
    get videos() { return videos; },
    get current() { return current; },
    get top() { return top; },
    get menuTitle() { return view.menuTitle; },
    get systemVolume() { return systemVolume; },
  };
}

export type PodController = ReturnType<typeof createPod>;
