import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc from "./schema";
import { selectChannel, setFavorite, setMuted, setVolume } from "./radio";
import { stationIds } from "./stations";

const station = s.enum(stationIds, { description: "A SomaFM station id from the bundled index." });

export default defineCommands(doc, {
  tune: {
    description: "Tune the receiver to a SomaFM station; playback still waits for the play button.",
    args: { station },
    run: ({ tx, current }, { station }) => selectChannel(tx, current, station),
  },
  setFavorite: {
    description: "Add or remove a station from the favorites the arrows cycle through and return whether it is a favorite.",
    args: { station, favorite: s.boolean() },
    run: ({ tx, current }, { station, favorite }) => setFavorite(tx, current, station, favorite),
  },
  setVolume: {
    description: "Set the gain from 0 to 1; a gain above zero also unmutes.",
    args: { volume: s.number({ description: "Gain, 0 to 1." }) },
    run: ({ tx, current }, { volume }) => setVolume(tx, current, volume),
  },
  setMuted: {
    description: "Mute or unmute the receiver.",
    args: { muted: s.boolean() },
    run: ({ tx }, { muted }) => setMuted(tx, muted),
  },
});
