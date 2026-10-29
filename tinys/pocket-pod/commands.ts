import { defineCommands } from "@hitslop/tinys/commands";
import { s } from "@hitslop/tinys";
import schema, { repeatModes, stickerKinds } from "./schema";
import {
  addSticker, addVideo, clearStickers, peelSticker, placeSticker, removeVideo, selectVideo,
  setOwnerName, setVolume, shuffleAll,
} from "./pod";

export default defineCommands(schema, {
  addVideo: {
    description: "Add a YouTube video to the library (the first one becomes current) and return its stable row ID.",
    args: {
      youtubeId: s.string({ description: "The 11-character YouTube video id, e.g. dQw4w9WgXcQ." }),
      title: s.string({ description: "The video's title." }),
      author: s.optional(s.string({ description: "The channel or artist name." })),
    },
    run: ({ tx, current }, { youtubeId, title, author }) => addVideo(tx, current, { youtubeId, title, author }),
  },
  selectVideo: {
    description: "Make a video the current one shown in Cover Flow and Now Playing.",
    args: { video: s.string({ description: "The video's $id." }) },
    run: ({ tx, current }, { video }) => selectVideo(tx, current, video),
  },
  removeVideo: {
    description: "Remove a video from the library; if it was current, its neighbour becomes current.",
    args: { video: s.string({ description: "The video's $id." }) },
    run: ({ tx, current }, { video }) => removeVideo(tx, current, video),
  },
  shuffle: {
    description: "Turn shuffle on, make a random video current and return its $id.",
    args: {},
    run: (ctx) => shuffleAll(ctx.tx, ctx.current, () => ctx.random()),
  },
  updateSettings: {
    description: "Change any of the player settings: volume, repeat, shuffle, clicker sound or owner name.",
    args: {
      volume: s.optional(s.number({ min: 0, max: 1, description: "Volume from 0 to 1." })),
      repeat: s.optional(s.enum(repeatModes, { description: "Repeat off, all videos, or one video." })),
      shuffle: s.optional(s.boolean({ description: "Play videos in random order." })),
      clicker: s.optional(s.boolean({ description: "Tick when the wheel turns." })),
      ownerName: s.optional(s.string({ maxLength: 24, description: "The owner's name; empty clears it." })),
    },
    run: ({ tx }, { volume, repeat, shuffle, clicker, ownerName }) => {
      if (volume !== undefined) setVolume(tx, volume);
      if (repeat !== undefined) tx.fields.repeat.set(repeat);
      if (shuffle !== undefined) tx.fields.shuffle.set(shuffle);
      if (clicker !== undefined) tx.fields.clicker.set(clicker);
      if (ownerName !== undefined) setOwnerName(tx, ownerName);
    },
  },
  addSticker: {
    description: "Stick a sticker on the case at a random spot and tilt and return its stable row ID.",
    args: { kind: s.enum(stickerKinds, { description: "Which sticker from the pack." }) },
    run: (ctx, { kind }) => addSticker(ctx.tx, kind, () => ctx.random()),
  },
  moveSticker: {
    description: "Move a sticker to a position given as fractions of the case width and height.",
    args: {
      sticker: s.string({ description: "The sticker's $id." }),
      x: s.number({ min: 0, max: 1, description: "0 is the left edge, 1 the right edge." }),
      y: s.number({ min: 0, max: 1, description: "0 is the top edge, 1 the bottom edge." }),
    },
    run: ({ tx, current }, { sticker, x, y }) => placeSticker(tx, current, sticker, x, y),
  },
  peelSticker: {
    description: "Peel one sticker off the case.",
    args: { sticker: s.string({ description: "The sticker's $id." }) },
    run: ({ tx, current }, { sticker }) => peelSticker(tx, current, sticker),
  },
  clearStickers: {
    description: "Peel every sticker off the case and return how many came off.",
    args: {},
    run: ({ tx, current }) => clearStickers(tx, current),
  },
});
