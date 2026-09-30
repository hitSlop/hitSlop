import { test, expect, afterEach } from "bun:test";
import { LookupError, youtube } from "../src/app/embed";

const relay = "https://hitslop.com";

test("video ids come from every common YouTube link and from pasted text", () => {
  const id = "dQw4w9WgXcQ";
  for (const link of [
    `https://www.youtube.com/watch?v=${id}`,
    `https://www.youtube.com/watch?v=${id}&list=PL123&t=42s`,
    `youtube.com/watch?v=${id}`,
    `https://m.youtube.com/watch?v=${id}`,
    `https://music.youtube.com/watch?v=${id}`,
    `https://youtu.be/${id}?si=abc`,
    `https://www.youtube.com/shorts/${id}`,
    `https://www.youtube.com/embed/${id}`,
    `https://www.youtube-nocookie.com/embed/${id}`,
    `https://www.youtube.com/live/${id}`,
  ]) expect(youtube.parseId(link)).toBe(id);
  expect(youtube.findId(`listen to this: https://youtu.be/${id} it's great`)).toBe(id);
  for (const text of ["", "photography", "https://example.com/watch?v=dQw4w9WgXcQ", "https://www.youtube.com/watch?v=short", "https://www.youtube.com/playlist?list=PL123", "not a url at all"]) {
    expect(youtube.parseId(text)).toBeNull();
  }
  expect(youtube.findId("nothing to see here")).toBeNull();
});

test("clip never splits a surrogate pair", () => {
  expect(youtube.clip("hello", 10)).toBe("hello");
  expect(youtube.clip("hello world", 5)).toBe("hello");
  // The emoji is two UTF-16 units; cutting between them would leave a lone surrogate.
  expect(youtube.clip("ab😀cd", 3)).toBe("ab");
  expect(youtube.clip("ab😀cd", 4)).toBe("ab😀");
});

test("the relay URL carries the video and options in the fragment", () => {
  expect(youtube.src("dQw4w9WgXcQ")).toBe("https://hitslop.com/embed/youtube.html#v=dQw4w9WgXcQ");
  expect(youtube.src("dQw4w9WgXcQ", { controls: true, autoplay: false, captions: true, start: 30 })).toBe(
    "https://hitslop.com/embed/youtube.html#v=dQw4w9WgXcQ&controls=1&autoplay=0&cc=1&start=30",
  );
  expect(youtube.src("dQw4w9WgXcQ", { relay: "https://example.com/relay.html", start: -4 })).toBe(
    "https://example.com/relay.html#v=dQw4w9WgXcQ",
  );
  expect(() => youtube.src("not an id")).toThrow();
});

const realFetch = globalThis.fetch;
afterEach(() => { globalThis.fetch = realFetch; });
const stubFetch = (respond: () => Response | Promise<Response>) => {
  globalThis.fetch = (async () => respond()) as unknown as typeof fetch;
};

test("lookup returns the title and author, and explains why a video can't be added", async () => {
  stubFetch(() => Response.json({ title: "Sandstorm", author_name: "Darude" }));
  expect(await youtube.lookup("y6120QOlsfU")).toEqual({ youtubeId: "y6120QOlsfU", title: "Sandstorm", author: "Darude" });
  stubFetch(() => Response.json({}));
  expect(await youtube.lookup("y6120QOlsfU")).toEqual({ youtubeId: "y6120QOlsfU", title: "Untitled video", author: "" });
  stubFetch(() => Response.json({ title: "x".repeat(300), author_name: "y".repeat(300) }));
  const long = await youtube.lookup("y6120QOlsfU");
  expect([long.title.length, long.author.length]).toEqual([200, 120]);

  for (const [status, kind] of [[401, "embed"], [403, "embed"], [400, "missing"], [404, "missing"], [500, "network"]] as const) {
    stubFetch(() => new Response("", { status }));
    const failure = await youtube.lookup("y6120QOlsfU").catch((error) => error);
    expect(failure).toBeInstanceOf(LookupError);
    expect(failure.kind).toBe(kind);
  }
  globalThis.fetch = (async () => { throw new TypeError("offline"); }) as unknown as typeof fetch;
  expect((await youtube.lookup("y6120QOlsfU").catch((error) => error)).kind).toBe("network");
});

// A frame and a window just real enough for the player's postMessage conversation.
function fixture(src = `${relay}/embed/youtube.html#v=dQw4w9WgXcQ`) {
  const windowListeners = new Set<(event: unknown) => void>();
  const frameListeners = new Set<() => void>();
  const sent: { payload: any; target: string }[] = [];
  const contentWindow = { postMessage: (data: string, target: string) => sent.push({ payload: JSON.parse(data), target }) };
  const frame = {
    src,
    contentWindow,
    addEventListener: (_: string, listener: () => void) => frameListeners.add(listener),
    removeEventListener: (_: string, listener: () => void) => frameListeners.delete(listener),
  } as unknown as HTMLIFrameElement;
  const fakeWindow = {
    addEventListener: (_: string, listener: (event: unknown) => void) => windowListeners.add(listener),
    removeEventListener: (_: string, listener: (event: unknown) => void) => windowListeners.delete(listener),
  };
  (globalThis as unknown as { window: unknown }).window = fakeWindow;
  const fromPlayer = (data: unknown, overrides: { source?: unknown; origin?: string } = {}) => {
    for (const listener of [...windowListeners]) {
      listener({ source: contentWindow, origin: relay, data: typeof data === "string" ? data : JSON.stringify(data), ...overrides });
    }
  };
  return { frame, sent, fromPlayer, windowListeners, frameListeners };
}

test("the player asks the relay to report, then turns its messages into events", () => {
  const { frame, sent, fromPlayer } = fixture();
  const log: string[] = [];
  const live = youtube.player(frame, {
    onReady: () => log.push("ready"),
    onState: (state) => log.push(`state ${state}`),
    onTime: (time, duration) => log.push(`time ${time}/${duration}`),
    onEnded: () => log.push("ended"),
    onError: (code) => log.push(`error ${code}`),
  });
  fromPlayer({ event: "initialDelivery", info: { playerState: -1 } });
  fromPlayer({ event: "onStateChange", info: 1 });
  fromPlayer({ event: "infoDelivery", info: { currentTime: 12.5, duration: 212 } });
  fromPlayer({ event: "infoDelivery", info: { currentTime: 13 } });
  fromPlayer({ event: "onStateChange", info: 0 });
  // The same end reported twice is one end.
  fromPlayer({ event: "infoDelivery", info: { playerState: 0 } });
  fromPlayer({ event: "onError", info: 101 });
  expect(log).toEqual([
    "ready", "state -1", "state 1", "time 12.5/212", "time 13/212", "state 0", "ended", "state 0", "error 101",
  ]);
  live.dispose();
  expect(sent).toEqual([]);
});

test("the handshake repeats until the player answers, and is addressed to the relay's origin", async () => {
  const { frame, sent, fromPlayer, frameListeners } = fixture();
  const player = youtube.player(frame);
  for (const listener of [...frameListeners]) listener();
  expect(sent).toEqual([{ payload: { event: "listening", id: 1, channel: "widget" }, target: relay }]);
  await new Promise((resolve) => setTimeout(resolve, 520));
  expect(sent.length).toBe(2);
  fromPlayer({ event: "onReady" });
  await new Promise((resolve) => setTimeout(resolve, 520));
  expect(sent.length).toBe(2);
  player.dispose();
});

test("messages from other windows or origins are ignored", () => {
  const { frame, fromPlayer } = fixture();
  const log: string[] = [];
  const player = youtube.player(frame, { onReady: () => log.push("ready"), onState: (state) => log.push(`state ${state}`) });
  fromPlayer({ event: "onStateChange", info: 1 }, { origin: "https://evil.example" });
  fromPlayer({ event: "onStateChange", info: 1 }, { source: {} });
  fromPlayer("not json");
  fromPlayer({ nothing: true });
  expect(log).toEqual([]);
  player.dispose();
});

test("commands go to the relay in the player's own format", () => {
  const { frame, sent } = fixture();
  const player = youtube.player(frame);
  player.play();
  player.pause();
  player.seek(42.5);
  player.seek(-3);
  player.setVolume(0.456);
  player.setVolume(7);
  expect(sent.map((message) => message.payload)).toEqual([
    { event: "command", func: "playVideo", args: [] },
    { event: "command", func: "pauseVideo", args: [] },
    { event: "command", func: "seekTo", args: [42.5, true] },
    { event: "command", func: "seekTo", args: [0, true] },
    { event: "command", func: "setVolume", args: [46] },
    { event: "command", func: "setVolume", args: [100] },
  ]);
  expect(sent.every((message) => message.target === relay)).toBe(true);
  player.dispose();
});

test("a relay that never answers is reported once, and a disposed player stays quiet", async () => {
  const silent = fixture();
  const codes: number[] = [];
  const player = youtube.player(silent.frame, { onError: (code) => codes.push(code) }, { timeoutMs: 30 });
  await new Promise((resolve) => setTimeout(resolve, 80));
  expect(codes).toEqual([-1]);
  player.dispose();

  const quiet = fixture();
  const late: number[] = [];
  const disposed = youtube.player(quiet.frame, { onError: (code) => late.push(code) }, { timeoutMs: 30 });
  disposed.dispose();
  await new Promise((resolve) => setTimeout(resolve, 80));
  expect(late).toEqual([]);
  expect(quiet.windowListeners.size).toBe(0);
  expect(quiet.frameListeners.size).toBe(0);

  const answered = fixture();
  const none: number[] = [];
  const heard = youtube.player(answered.frame, { onError: (code) => none.push(code) }, { timeoutMs: 30 });
  answered.fromPlayer({ event: "onReady" });
  await new Promise((resolve) => setTimeout(resolve, 80));
  expect(none).toEqual([]);
  heard.dispose();
});
