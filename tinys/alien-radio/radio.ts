import { refuse, type Tx, type Value } from "@hitslop/tinys";
import type schema from "./schema";
import { fallbackChannels, type Channel } from "./stations";

export type Radio = Value<typeof schema.descriptor>;
type Edit = Tx<typeof schema>;

const stationId = /^[a-z0-9-]{1,64}$/;

/** The editor and its captures resolve the tuned station the same way. `channels` is the
 * live catalog when the page has one, else the bundled index. An id that neither lists
 * falls back to the first station. */
export function radioView(data: Radio, channels: readonly Channel[] = fallbackChannels) {
  const selected = channels.find(channel => channel.id === data.selectedChannelId)
    ?? fallbackChannels.find(channel => channel.id === data.selectedChannelId)
    ?? channels[0] ?? fallbackChannels[0]!;
  const favorites = channels.filter(channel => data.favoriteChannelIds.includes(channel.id));
  return {
    selected,
    isFavorite: data.favoriteChannelIds.includes(selected.id),
    favorites,
    gain: Math.round(data.volume * 100),
  };
}

/** The station the transport arrows tune to: the next favorite, or the next station when
 * none of the favorites is listed. */
export function stepChannel(data: Radio, channels: readonly Channel[], direction: -1 | 1): Channel | undefined {
  const { selected, favorites } = radioView(data, channels);
  const pool = favorites.length ? favorites : channels;
  if (!pool.length) return undefined;
  const index = Math.max(0, pool.findIndex(channel => channel.id === selected.id));
  return pool[(index + direction + pool.length) % pool.length];
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.
export function selectChannel(tx: Edit, current: Radio, id: string) {
  if (!stationId.test(id)) refuse("Enter a SomaFM station id, such as groovesalad.");
  if (id !== current.selectedChannelId) tx.fields.selectedChannelId.set(id);
}

/** Marks or unmarks a station as a favorite; returns whether it is now a favorite. */
export function setFavorite(tx: Edit, current: Radio, id: string, favorite: boolean) {
  if (!stationId.test(id)) refuse("Enter a SomaFM station id, such as groovesalad.");
  const index = current.favoriteChannelIds.indexOf(id);
  if (favorite && index < 0) tx.fields.favoriteChannelIds.insert(id);
  else if (!favorite && index >= 0) tx.fields.favoriteChannelIds.remove(index);
  return favorite;
}

/** Sets the gain from 0 to 1; raising it above zero also unmutes, as the slider does. */
export function setVolume(tx: Edit, current: Radio, volume: number) {
  if (!Number.isFinite(volume) || volume < 0 || volume > 1) refuse("Volume runs from 0 to 1.");
  tx.fields.volume.set(volume);
  if (volume > 0 && current.muted) tx.fields.muted.set(false);
}

export function setMuted(tx: Edit, muted: boolean) {
  tx.fields.muted.set(muted);
}
