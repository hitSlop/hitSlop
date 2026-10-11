import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import doc, { bands } from "./schema";
import { stationIds } from "./stations";
import { selectStation, setBalance, setBand, setEqualizerOn, setVolume, useBaseSkin } from "./settings";

export default defineCommands(doc, {
  selectStation: {
    description: "Tune the receiver to one of the eight SomaFM stations; playback stays paused until someone presses play.",
    args: { station: s.enum(stationIds, { description: "The station ID, e.g. groovesalad or dronezone." }) },
    run: ({ tx }, { station }) => selectStation(tx, station),
  },
  setVolume: {
    description: "Set the saved volume from 0 to 100.",
    args: { volume: s.number({ description: "Volume, 0 to 100." }) },
    run: ({ tx }, { volume }) => setVolume(tx, volume),
  },
  setBalance: {
    description: "Set the left/right balance from -100 (left) to 100 (right); 0 is centered.",
    args: { balance: s.number({ description: "Balance, -100 to 100." }) },
    run: ({ tx }, { balance }) => setBalance(tx, balance),
  },
  setEqualizerOn: {
    description: "Turn the equalizer on or off.",
    args: { on: s.boolean() },
    run: ({ tx }, { on }) => setEqualizerOn(tx, on),
  },
  setEqualizerBand: {
    description: "Set the preamp or one EQ band from 0 to 100; 50 is flat.",
    args: {
      band: s.enum(bands, { description: "preamp, or a band such as hz60 or hz16000." }),
      value: s.number({ description: "Slider position, 0 to 100." }),
    },
    run: ({ tx }, { band, value }) => setBand(tx, band, value),
  },
  useBaseSkin: {
    description: "Return to the classic base skin and return whether an imported skin was cleared.",
    args: {},
    run: ({ tx, current }) => useBaseSkin(tx, current),
  },
});
