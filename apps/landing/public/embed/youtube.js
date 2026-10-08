// hitSlop YouTube relay.
//
// YouTube refuses to play for a page that doesn't send an HTTP(S) referrer, and a slop's own
// page (slop://app) has none. A slop embeds this page instead; this page embeds the player, so
// YouTube sees https://hitslop.com as the embedding site. Nothing here is specific to one slop:
// the video comes from the URL fragment (never sent to our server), and messages are relayed
// between the slop and the player. The player's own postMessage API is the only thing exposed.
//
//   https://hitslop.com/embed/youtube.html#v=<11-character id>[&controls=1][&autoplay=0][&start=<seconds>][&cc=1]
//
// Use the SDK helper (`hitslop/embed`) instead of writing this URL by hand.
const playerOrigin = "https://www.youtube-nocookie.com";
const commands = new Set(["playVideo", "pauseVideo", "seekTo", "setVolume", "mute", "unMute"]);

const params = new URLSearchParams(location.hash.slice(1));
const id = params.get("v") ?? "";
const flag = (name, fallback) => (params.get(name) === "1" ? "1" : params.get(name) === "0" ? "0" : fallback);

if (/^[A-Za-z0-9_-]{11}$/.test(id)) {
  const query = new URLSearchParams({
    enablejsapi: "1",
    playsinline: "1",
    rel: "0",
    modestbranding: "1",
    iv_load_policy: "3",
    disablekb: "1",
    fs: "0",
    controls: flag("controls", "0"),
    autoplay: flag("autoplay", "1"),
  });
  const start = Number.parseInt(params.get("start") ?? "", 10);
  if (Number.isSafeInteger(start) && start > 0) query.set("start", String(start));
  const captions = flag("cc", "0") === "1";

  const player = document.createElement("iframe");
  player.title = "YouTube video player";
  player.allow = "autoplay; encrypted-media";
  player.referrerPolicy = "strict-origin-when-cross-origin";
  player.src = `${playerOrigin}/embed/${id}?${query}`;
  document.body.append(player);

  const toPlayer = (payload) => player.contentWindow?.postMessage(JSON.stringify(payload), playerOrigin);
  const parse = (data) => {
    try {
      const message = typeof data === "string" ? JSON.parse(data) : data;
      return message && typeof message === "object" ? message : null;
    } catch {
      return null;
    }
  };

  // Captions are the player's own track and it can load after the player is ready, so keep
  // turning it off until it has played for a moment, unless the slop asked to see it.
  const hideCaptions = () => {
    toPlayer({ event: "command", func: "unloadModule", args: ["captions"] });
    toPlayer({ event: "command", func: "setOption", args: ["captions", "track", {}] });
  };
  let hidden = 0;
  window.addEventListener("message", (event) => {
    if (event.source === player.contentWindow && event.origin === playerOrigin) {
      const message = parse(event.data);
      if (!captions && hidden < 6 && (message?.event === "onReady" || message?.event === "onStateChange" || message?.event === "infoDelivery")) {
        hidden += 1;
        hideCaptions();
      }
      window.parent.postMessage(event.data, "*");
      return;
    }
    if (event.source !== window.parent || window.parent === window) return;
    const message = parse(event.data);
    if (!message) return;
    if (message.event === "listening") {
      toPlayer({ event: "listening", id: 1, channel: "widget" });
    } else if (message.event === "command" && commands.has(message.func) && Array.isArray(message.args)) {
      const args = message.args.filter((value) => typeof value === "boolean" || Number.isFinite(value)).slice(0, 2);
      toPlayer({ event: "command", func: message.func, args });
    }
  });
}
window.addEventListener("hashchange", () => location.reload());
