import type Webamp from "webamp";
import type { Slider } from "webamp";
import { bands, type SomaAmp } from "./schema";
import { stationTrack, type ResolvedStation } from "./stations";

export type AudioPreferences = Pick<SomaAmp, "volume" | "balance" | "equalizer">;
const clamp = (n: number, low = 0, high = 100) => Math.max(low, Math.min(high, n));
const slider = (key: typeof bands[number]) => (key === "preamp" ? key : Number(key.slice(2))) as Slider;

/** All pinned Webamp 2.3.1 internals live here; document state never mirrors its Redux store. */
export class Receiver {
  private cleanup: Array<() => void> = [];
  private disposed = false;
  private syncing = false;
  private settingSkin = false;
  private cancelSkin?: () => void;
  private lastAudio = "";
  private constructor(readonly webamp: Webamp) {}
  /** `skin` is a validated saved skin, applied before first paint; `skinApplied` reports whether Webamp accepted it. */
  static async create(node: HTMLElement, stations: ResolvedStation[], preferences: SomaAmp, skin: Blob | undefined, callbacks: {
    audio(value: AudioPreferences): void;
    station(id: string): void;
    visualizer(open: boolean): void;
    baseSkin(): void;
    status(message: string, error?: boolean): void;
  }, signal: AbortSignal) {
    const { default: Webamp } = await import("webamp/butterchurn");
    signal.throwIfAborted();
    const skinURL = skin && URL.createObjectURL(skin);
    const webamp = new Webamp({
      initialTracks: stations.map(stationTrack),
      ...(skinURL ? { initialSkin: { url: skinURL } } : {}),
      enableDoubleSizeMode: false,
      enableHotkeys: true,
      enableMediaSession: true,
      // Radio presets are the playlist; do not offer temporary local-audio imports.
      handleTrackDropEvent: async () => [],
      handleAddUrlEvent: async () => [],
      handleLoadListEvent: async () => [],
      windowLayout: {
        main: { position: { top: 0, left: 0 } },
        equalizer: { position: { top: 116, left: 0 } },
        playlist: { position: { top: 232, left: 0 }, size: { extraHeight: 2, extraWidth: 0 } },
        milkdrop: { position: { top: 0, left: 275 }, size: { extraHeight: 10, extraWidth: 7 }, closed: true },
      },
    });
    const receiver = new Receiver(webamp);
    // Webamp falls back to its base skin when the initial skin fails; SET_SKIN_DATA marks success.
    let skinApplied = false;
    const stopSkin = webamp._actionEmitter.on("SET_SKIN_DATA", () => { skinApplied = true; });
    const abort = () => receiver.dispose();
    signal.addEventListener("abort", abort, { once: true });
    receiver.cleanup.push(() => signal.removeEventListener("abort", abort));
    try {
      try { await webamp.renderInto(node); } finally { stopSkin(); if (skinURL) URL.revokeObjectURL(skinURL); }
      signal.throwIfAborted();
      receiver.applyAudio(preferences);
      const media = webamp.media as typeof webamp.media & { _source: { _audio: HTMLAudioElement } };
      const audio = media._source._audio;
      const failed = (event: Event) => {
        event.stopImmediatePropagation();
        webamp.stop();
        callbacks.status("Stream unavailable. Try another station or press play to retry.", true);
      };
      const waiting = () => callbacks.status("Connecting to SomaFM…");
      const playing = () => callbacks.status("Live · SomaFM");
      audio.addEventListener("error", failed, true);
      audio.addEventListener("waiting", waiting);
      audio.addEventListener("playing", playing);
      receiver.cleanup.push(() => {
        audio.removeEventListener("error", failed, true);
        audio.removeEventListener("waiting", waiting);
        audio.removeEventListener("playing", playing);
      });
      webamp.setCurrentTrack(Math.max(0, stations.findIndex(s => s.id === preferences.selectedStationId)));
      receiver.cleanup.push(
        webamp.onWillClose(cancel => cancel()),
        webamp.onTrackDidChange(track => {
          const station = stations.find(s => s.stream === track?.url);
          if (station && !receiver.syncing) callbacks.station(station.id);
        }),
        webamp._actionEmitter.on("LOAD_DEFAULT_SKIN", () => { if (!receiver.settingSkin) callbacks.baseSkin(); }),
        webamp._actionEmitter.on("SET_MEDIA", () => callbacks.status("Ready · press play")),
        webamp._actionEmitter.on("PAUSE", () => callbacks.status("Paused")),
        webamp._actionEmitter.on("STOP", () => callbacks.status("Stopped")),
      );
      let previousVis = false;
      receiver.cleanup.push(webamp.store.subscribe(() => {
        if (receiver.disposed || receiver.syncing) return;
        const audio = receiver.audio();
        const key = JSON.stringify(audio);
        if (key !== receiver.lastAudio) { receiver.lastAudio = key; callbacks.audio(audio); }
        const open = receiver.visualizerOpen;
        if (open !== previousVis) { previousVis = open; queueMicrotask(() => { if (!receiver.disposed) callbacks.visualizer(open); }); }
      }));
      return { receiver, skinApplied };
    } catch (error) { receiver.dispose(); throw error; }
  }
  audio(): AudioPreferences {
    const { media, equalizer } = this.webamp.store.getState();
    const eq = Object.fromEntries(bands.map(key => [key, equalizer.sliders[slider(key)]]));
    return { volume: media.volume, balance: media.balance, equalizer: { ...eq, on: equalizer.on } as SomaAmp["equalizer"] };
  }
  applyAudio(value: AudioPreferences) {
    if (this.disposed) return;
    this.syncing = true;
    try {
      const state = this.webamp.store.getState();
      if (state.media.volume !== clamp(value.volume)) this.webamp.setVolume(clamp(value.volume));
      if (state.media.balance !== clamp(value.balance, -100, 100))
        this.webamp.store.dispatch({ type: "SET_BALANCE", balance: clamp(value.balance, -100, 100) });
      if (state.equalizer.on !== value.equalizer.on)
        this.webamp.store.dispatch({ type: value.equalizer.on ? "SET_EQ_ON" : "SET_EQ_OFF" });
      for (const key of bands) if (state.equalizer.sliders[slider(key)] !== clamp(value.equalizer[key]))
        this.webamp.store.dispatch({ type: "SET_BAND_VALUE", band: slider(key), value: clamp(value.equalizer[key]) });
      this.lastAudio = JSON.stringify(this.audio());
    } finally { this.syncing = false; }
  }
  selectStation(id: string, stations: ResolvedStation[]) {
    const index = stations.findIndex(s => s.id === id);
    const state = this.webamp.store.getState();
    if (index < 0 || state.playlist.currentTrack === state.playlist.trackOrder[index]) return;
    this.syncing = true;
    try { this.webamp.setCurrentTrack(index); } finally { this.syncing = false; }
  }
  get visualizerOpen() { return Boolean(this.webamp.store.getState().windows.genWindows.milkdrop?.open); }
  setVisualizer(open: boolean) {
    if (open !== this.visualizerOpen)
      this.webamp.store.dispatch({ type: open ? "TOGGLE_WINDOW" : "CLOSE_WINDOW", windowId: "milkdrop" });
  }
  reducedMotion(reduced: boolean) {
    this.webamp.store.dispatch({ type: reduced ? "DISABLE_MARQUEE" : "ENABLE_MARQUEE" });
    if (reduced) this.setVisualizer(false);
  }
  baseSkin() {
    this.settingSkin = true;
    this.webamp.store.dispatch({ type: "LOAD_DEFAULT_SKIN" });
    this.settingSkin = false;
  }
  async loadSkin(blob: Blob) {
    if (this.disposed) throw new Error("Receiver closed");
    this.cancelSkin?.();
    const url = URL.createObjectURL(blob);
    this.settingSkin = true;
    try {
      await new Promise<void>((resolve, reject) => {
        const finish = (error?: Error) => {
          clearTimeout(timer); stopLoaded(); stopFailed(); this.cancelSkin = undefined;
          error ? reject(error) : resolve();
        };
        const stopLoaded = this.webamp._actionEmitter.on("SET_SKIN_DATA", () => finish());
        const stopFailed = this.webamp._actionEmitter.on("LOADED", () => finish(new Error("Webamp could not apply this skin.")));
        const timer = setTimeout(() => finish(new Error("The skin took too long to load.")), 15000);
        this.cancelSkin = () => finish(new Error("Skin loading cancelled"));
        this.webamp.setSkinFromUrl(url);
      });
    } finally { this.settingSkin = false; URL.revokeObjectURL(url); }
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.cancelSkin?.();
    this.cleanup.splice(0).forEach(stop => stop());
    this.webamp.stop();
    this.webamp.dispose();
    const media = this.webamp.media as typeof this.webamp.media & { _source: { _audio: HTMLAudioElement } };
    media._source._audio.removeAttribute("src");
    media._source._audio.load();
    void (media.getAnalyser().context as AudioContext).close().catch(() => {});
  }
}
