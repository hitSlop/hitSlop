import { refuse, type Tx, type Value } from "hitslop";
import type schema from "./schema";
import { kinds, maxProps, maxSeed, seasons, type Kind, type Season } from "./schema";
import { SEA, direction, radiusAt } from "./terrain";

export type World = Value<typeof schema.descriptor>;
type Edit = Tx<typeof schema>;

const MAX_LAT = 1.5708;
const MAX_LON = 3.1416;

/** The editor and its export read the same summary of the saved world. */
export function worldView(data: World) {
  const count = data.props.length;
  return {
    count,
    full: count >= maxProps,
    title: data.name.trim() || "Tiny Planet",
    things: `${count} ${count === 1 ? "thing" : "things"}`,
    season: data.season,
  };
}

/** Why a thing cannot stand at (lat, lon) on this world, or undefined when it can. */
export function placeProblem(data: World, lat: number, lon: number): string | undefined {
  if (data.props.length >= maxProps) return "The planet is full. Remove something first.";
  if (!Number.isFinite(lat) || !Number.isFinite(lon) || Math.abs(lat) > MAX_LAT || Math.abs(lon) > MAX_LON)
    return `Latitude must be within ±${MAX_LAT} and longitude within ±${MAX_LON} radians.`;
  const [x, y, z] = direction(lat, lon);
  if (radiusAt(x, y, z, data.seed) <= SEA + 0.004) return "That’s open water. Try the land.";
  return undefined;
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function placeThing(tx: Edit, data: World, kind: Kind, lat: number, lon: number) {
  if (!kinds.includes(kind)) refuse(`Choose one of: ${kinds.join(", ")}.`);
  const problem = placeProblem(data, lat, lon);
  if (problem) refuse(problem);
  return tx.fields.props.insert({ kind, lat, lon });
}

export function removeThing(tx: Edit, id: string) {
  tx.fields.props.remove(id);
}

export function setSeason(tx: Edit, season: Season) {
  if (!seasons.includes(season)) refuse(`Choose one of: ${seasons.join(", ")}.`);
  tx.fields.season.set(season);
}

export function rename(tx: Edit, name: string) {
  tx.fields.name.set(name.trim().slice(0, 32));
}

/** Roll a different seed from `random` (0 ≤ r < 1). Things keep their latitude and longitude. */
export function newWorld(tx: Edit, data: World, random: () => number) {
  let seed = data.seed;
  for (let tries = 0; seed === data.seed && tries < 16; tries++) seed = Math.floor(random() * (maxSeed + 1));
  if (seed === data.seed) seed = (data.seed + 1) % (maxSeed + 1);
  tx.fields.seed.set(seed);
  return seed;
}
