// Embedding third-party players. A slop's page (`slop://app`) has no web identity, and YouTube's
// player refuses to play for an embedder that doesn't send an HTTP(S) referrer (error 153). So
// the player is embedded through a small relay page on hitslop.com, which supplies the referrer
// and forwards the player's postMessage API. Authors never run a server and never load YouTube's
// own script (remote scripts are blocked): they call `youtube.src` and `youtube.player`.

const defaultRelay = "https://hitslop.com/embed/youtube.html";
const idShape = /^[A-Za-z0-9_-]{11}$/;
const youtubeHosts = new Set([
  "youtube.com", "www.youtube.com", "m.youtube.com", "music.youtube.com",
  "youtube-nocookie.com", "www.youtube-nocookie.com",
]);

export type VideoMeta = { youtubeId: string; title: string; author: string };

export type YouTubeSrcOptions = {
  /** Show YouTube's own controls. Off by default: the slop draws its own. */
  controls?: boolean;
  /** Start playing as soon as the player loads. On by default. */
  autoplay?: boolean;
  /** Show the video's caption track, if it has one. Off by default. */
  captions?: boolean;
  /** Start position in whole seconds. */
  start?: number;
  /** A relay you host yourself instead of hitslop.com. */
  relay?: string;
};

export type YouTubeHandlers = {
  /** The player answered for the first time. */
  onReady?(): void;
  /** `-1` unstarted, `0` ended, `1` playing, `2` paused, `3` buffering, `5` cued. */
  onState?(state: number): void;
  /** Playback position in seconds; `duration` is known once the player reports it. */
  onTime?(time: number, duration: number | undefined): void;
  /** The video played to its end. */
  onEnded?(): void;
  /** A YouTube error code (`100` missing, `101`/`150` embedding disabled, `153` no referrer), or
   * `-1` when the relay never answered, for example without a network. */
  onError?(code: number): void;
};

export type YouTubePlayer = {
  play(): void;
  pause(): void;
  seek(seconds: number): void;
  /** `0` to `1`. */
  setVolume(level: number): void;
  /** Stop listening. Call when the frame is removed. */
  dispose(): void;
};

export class LookupError extends Error {
  constructor(readonly kind: "missing" | "embed" | "network", message: string) {
    super(message);
  }
}

/** The 11-character video id in any common YouTube link, or null. Playlists are ignored. */
function parseId(input: string): string | null {
  const text = input.trim();
  let url: URL;
  try {
    url = new URL(/^[a-z][a-z0-9+.-]*:/i.test(text) ? text : `https://${text}`);
  } catch {
    return null;
  }
  const host = url.hostname.toLowerCase();
  let candidate: string | null = null;
  if (host === "youtu.be") candidate = url.pathname.split("/")[1] ?? null;
  else if (youtubeHosts.has(host)) {
    const [, kind = "", id] = url.pathname.split("/");
    candidate = kind === "watch" ? url.searchParams.get("v")
      : ["embed", "shorts", "live", "v"].includes(kind) ? id ?? null : null;
  }
  return candidate && idShape.test(candidate) ? candidate : null;
}

/** The first YouTube link in free text, such as a paste that carries more than the URL. */
function findId(text: string): string | null {
  for (const word of text.split(/\s+/)) {
    const id = parseId(word);
    if (id) return id;
  }
  return null;
}

/** Truncate to `max` UTF-16 units without splitting a surrogate pair. */
function clip(text: string, max: number): string {
  if (text.length <= max) return text;
  return text.slice(0, (text.charCodeAt(max - 1) & 0xfc00) === 0xd800 ? max - 1 : max);
}

const thumbnail = (youtubeId: string) => `https://i.ytimg.com/vi/${youtubeId}/mqdefault.jpg`;

/** Title and author from YouTube's oEmbed endpoint, which also refuses videos that can't be embedded. */
async function lookup(youtubeId: string, signal?: AbortSignal): Promise<VideoMeta> {
  const watch = encodeURIComponent(`https://www.youtube.com/watch?v=${youtubeId}`);
  let response: Response;
  try {
    response = await fetch(`https://www.youtube.com/oembed?format=json&url=${watch}`, { signal });
  } catch (cause) {
    if (signal?.aborted) throw cause;
    throw new LookupError("network", "Can't reach YouTube.");
  }
  if (response.status === 401 || response.status === 403) throw new LookupError("embed", "That video can't be embedded.");
  if (response.status === 400 || response.status === 404) throw new LookupError("missing", "No video found at that link.");
  if (!response.ok) throw new LookupError("network", "YouTube didn't answer. Try again.");
  const body = await response.json() as { title?: unknown; author_name?: unknown };
  return {
    youtubeId,
    title: clip(typeof body.title === "string" && body.title ? body.title : "Untitled video", 200),
    author: clip(typeof body.author_name === "string" ? body.author_name : "", 120),
  };
}

/** The relay URL to use as an `<iframe src>`. The video travels in the fragment, which is never sent to a server. */
function src(youtubeId: string, options: YouTubeSrcOptions = {}): string {
  if (!idShape.test(youtubeId)) throw new Error("A YouTube video id is 11 letters, digits, - or _");
  const parts = [`v=${youtubeId}`];
  if (options.controls !== undefined) parts.push(`controls=${options.controls ? 1 : 0}`);
  if (options.autoplay !== undefined) parts.push(`autoplay=${options.autoplay ? 1 : 0}`);
  if (options.captions) parts.push("cc=1");
  if (options.start !== undefined && Number.isSafeInteger(options.start) && options.start > 0) parts.push(`start=${options.start}`);
  return `${options.relay ?? defaultRelay}#${parts.join("&")}`;
}

type PlayerEvent =
  | { type: "state"; state: number }
  | { type: "time"; time?: number; duration?: number }
  | { type: "error"; code: number }
  | { type: "other" };

/** The player's messages as events. Anything unrecognized is `other`, which still counts as an answer. */
function decode(raw: unknown): PlayerEvent[] {
  let message = raw as { event?: unknown; info?: unknown } | null;
  if (typeof raw === "string") {
    try { message = JSON.parse(raw); } catch { return []; }
  }
  if (!message || typeof message !== "object" || typeof message.event !== "string") return [];
  if (message.event === "onStateChange" && typeof message.info === "number") return [{ type: "state", state: message.info }];
  if (message.event === "onError") return [{ type: "error", code: Number(message.info) }];
  if (message.event === "infoDelivery" || message.event === "initialDelivery") {
    const info = (message.info ?? {}) as Record<string, unknown>;
    const events: PlayerEvent[] = [];
    if (typeof info.playerState === "number") events.push({ type: "state", state: info.playerState });
    const time = typeof info.currentTime === "number" ? info.currentTime : undefined;
    const duration = typeof info.duration === "number" && info.duration > 0 ? info.duration : undefined;
    if (time !== undefined || duration !== undefined) events.push({ type: "time", time, duration });
    return events.length ? events : [{ type: "other" }];
  }
  return [{ type: "other" }];
}

/**
 * Control the player in `frame`, whose `src` came from `youtube.src`. Attach it after the frame
 * has its `src`; it keeps asking the relay to start reporting until the player answers.
 */
function player(
  frame: HTMLIFrameElement,
  handlers: YouTubeHandlers = {},
  options: { timeoutMs?: number } = {},
): YouTubePlayer {
  let origin: string;
  try {
    origin = new URL(frame.src || defaultRelay).origin;
  } catch {
    origin = new URL(defaultRelay).origin;
  }
  let heard = false;
  let state = -1;
  let duration: number | undefined;
  const post = (payload: object) => frame.contentWindow?.postMessage(JSON.stringify(payload), origin);
  const listen = () => { if (!heard) post({ event: "listening", id: 1, channel: "widget" }); };
  const answered = () => {
    if (heard) return;
    heard = true;
    clearTimeout(timeout);
    handlers.onReady?.();
  };
  const onMessage = (event: MessageEvent) => {
    if (event.source !== frame.contentWindow || event.origin !== origin) return;
    const events = decode(event.data);
    if (!events.length) return;
    answered();
    for (const item of events) {
      if (item.type === "state") {
        const before = state;
        state = item.state;
        handlers.onState?.(item.state);
        if (item.state === 0 && before !== 0) handlers.onEnded?.();
      } else if (item.type === "time") {
        if (item.duration !== undefined) duration = item.duration;
        if (item.time !== undefined) handlers.onTime?.(item.time, duration);
      } else if (item.type === "error") {
        handlers.onError?.(item.code);
      }
    }
  };
  window.addEventListener("message", onMessage);
  frame.addEventListener("load", listen);
  const poll = setInterval(listen, 400);
  const timeout = setTimeout(() => { if (!heard) handlers.onError?.(-1); }, options.timeoutMs ?? 10_000);
  return {
    play: () => post({ event: "command", func: "playVideo", args: [] }),
    pause: () => post({ event: "command", func: "pauseVideo", args: [] }),
    seek: (seconds) => post({ event: "command", func: "seekTo", args: [Math.max(0, seconds), true] }),
    setVolume: (level) => post({ event: "command", func: "setVolume", args: [Math.round(Math.min(1, Math.max(0, level)) * 100)] }),
    dispose() {
      clearInterval(poll);
      clearTimeout(timeout);
      window.removeEventListener("message", onMessage);
      frame.removeEventListener("load", listen);
    },
  };
}

export const youtube = { parseId, findId, thumbnail, lookup, src, player, clip };
