import { defineCommands } from "hitslop/commands";
import { s } from "hitslop";
import schema, { kinds, seasons } from "./schema";
import { newWorld, placeThing, removeThing, rename, setSeason } from "./world";

export default defineCommands(schema, {
  placeThing: {
    description: "Stand a tree, house, rock or crystal on land at a latitude and longitude in radians and return its stable row ID; open water and a full planet are refused.",
    args: {
      kind: s.enum(kinds),
      lat: s.number({ description: "Latitude in radians, from -1.5708 (south pole) to 1.5708 (north pole)." }),
      lon: s.number({ description: "Longitude in radians, from -3.1416 to 3.1416." }),
    },
    run: ({ tx, current }, { kind, lat, lon }) => placeThing(tx, current, kind, lat, lon),
  },
  removeThing: {
    description: "Remove a thing from the planet.",
    args: { thing: s.string({ description: "The thing's $id." }) },
    run: ({ tx }, { thing }) => removeThing(tx, thing),
  },
  setSeason: {
    description: "Change the planet's season, which recolors the ground and trees.",
    args: { season: s.enum(seasons) },
    run: ({ tx }, { season }) => setSeason(tx, season),
  },
  rename: {
    description: "Rename the planet (up to 32 characters).",
    args: { name: s.string() },
    run: ({ tx }, { name }) => rename(tx, name),
  },
  newWorld: {
    description: "Roll a new terrain seed from the command's random source, keep every thing at its latitude and longitude, and return the new seed.",
    args: {},
    run: ({ tx, current, random }) => newWorld(tx, current, random),
  },
});
