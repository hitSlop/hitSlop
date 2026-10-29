import { refuse, type Tx, type Value } from "@hitslop/tinys";
import type schema from "./schema";
import { bands } from "./schema";
import { stations, type Station } from "./station-list";

export type Receiver = Value<typeof schema.descriptor>;
export type Band = (typeof bands)[number];
export type AudioPreferences = Pick<Receiver, "volume" | "balance" | "equalizer">;
type Edit = Tx<typeof schema>;

const stationOf = (id: string): Station | undefined => stations.find(station => station.id === id);

/** The receiver and its export read the station, skin and settings from the same snapshot. */
export function receiverView(data: Receiver) {
  return {
    station: stationOf(data.selectedStationId) ?? stations[0]!,
    skinName: data.skin.name ?? "Classic base skin",
    hasSkin: Boolean(data.skin.id),
    volume: Math.round(data.volume),
    eqOn: data.equalizer.on,
  };
}

function inRange(value: number, low: number, high: number, label: string) {
  if (!Number.isFinite(value) || value < low || value > high) refuse(`${label} must be between ${low} and ${high}.`);
  return value;
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function selectStation(tx: Edit, id: string) {
  const station = stationOf(id);
  if (!station) refuse(`Unknown station "${id}". Choose one of: ${stations.map(s => s.id).join(", ")}.`);
  tx.fields.selectedStationId.set(station.id);
}
export function setVolume(tx: Edit, volume: number) {
  tx.fields.volume.set(inRange(volume, 0, 100, "Volume"));
}
export function setBalance(tx: Edit, balance: number) {
  tx.fields.balance.set(inRange(balance, -100, 100, "Balance"));
}
export function setEqualizerOn(tx: Edit, on: boolean) {
  tx.fields.equalizer.on.set(on);
}
export function setBand(tx: Edit, band: Band, value: number) {
  if (!bands.includes(band)) refuse(`Unknown band "${band}".`);
  tx.fields.equalizer[band].set(inRange(value, 0, 100, band === "preamp" ? "Preamp" : "An EQ band"));
}
/** Commit the volume, balance and EQ the receiver settled on after a drag. */
export function saveAudio(tx: Edit, value: AudioPreferences) {
  setVolume(tx, value.volume);
  setBalance(tx, value.balance);
  setEqualizerOn(tx, value.equalizer.on);
  for (const band of bands) setBand(tx, band, value.equalizer[band]);
}
export function setVisualizer(tx: Edit, open: boolean) {
  tx.fields.milkdropOpen.set(open);
}
export function setSkin(tx: Edit, ref: { id: string; name: string }) {
  tx.fields.skin.id.set(ref.id);
  tx.fields.skin.name.set(ref.name);
}
/** Return to Webamp's built-in base skin. The imported file is removed when the document
 * closes, because nothing references it any more. Returns whether a skin was cleared. */
export function useBaseSkin(tx: Edit, current: Receiver) {
  if (!current.skin.id && !current.skin.name) return false;
  tx.fields.skin.id.clear();
  tx.fields.skin.name.clear();
  return true;
}
