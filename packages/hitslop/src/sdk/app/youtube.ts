// YouTube URLs as strings: a video's thumbnail and the relay URL a frame embeds. Nothing here needs
// the page, the network or a web API, so unlike `hitslop/embed` (reading a pasted link, the title
// lookup and the player) a static view (Preview, Export, Icon, Widget) may import it.

/** The relay page `src` points a frame at, unless its `relay` option names another. */
export const defaultRelay = "https://hitslop.com/embed/youtube.html";
/** What a video id looks like: 11 letters, digits, `-` or `_`. */
export const idShape = /^[A-Za-z0-9_-]{11}$/;

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

/** The video's thumbnail image. */
export const thumbnail = (youtubeId: string) => `https://i.ytimg.com/vi/${youtubeId}/mqdefault.jpg`;

/** The relay URL to use as an `<iframe src>`. The video travels in the fragment, which is never sent to a server. */
export function src(youtubeId: string, options: YouTubeSrcOptions = {}): string {
  if (!idShape.test(youtubeId)) throw new Error("A YouTube video id is 11 letters, digits, - or _");
  const parts = [`v=${youtubeId}`];
  if (options.controls !== undefined) parts.push(`controls=${options.controls ? 1 : 0}`);
  if (options.autoplay !== undefined) parts.push(`autoplay=${options.autoplay ? 1 : 0}`);
  if (options.captions) parts.push("cc=1");
  if (options.start !== undefined && Number.isSafeInteger(options.start) && options.start > 0)
    parts.push(`start=${options.start}`);
  return `${options.relay ?? defaultRelay}#${parts.join("&")}`;
}
