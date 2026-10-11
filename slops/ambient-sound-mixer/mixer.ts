import { refuse, type Tx } from "hitslop";
import { CHANNEL_IDS } from "./schema";
import type doc from "./schema";

export type Mixer = typeof doc.current;
type Edit = Tx<typeof doc>;
export type ChannelID = (typeof CHANNEL_IDS)[number];
export type ChannelLevels = Record<ChannelID, number>;
export type PresetName = "Lo-Fi Rain" | "Forest Dawn" | "Midnight Storm" | "Cozy Evening";

export const PRESETS: { name: PresetName; levels: ChannelLevels }[] = [
  { name: "Lo-Fi Rain", levels: { rain: 75, thunder: 20, wind: 35, birds: 0, night: 40 } },
  { name: "Forest Dawn", levels: { rain: 15, thunder: 0, wind: 40, birds: 85, night: 10 } },
  { name: "Midnight Storm", levels: { rain: 85, thunder: 70, wind: 60, birds: 0, night: 0 } },
  { name: "Cozy Evening", levels: { rain: 30, thunder: 0, wind: 25, birds: 20, night: 75 } },
];
export const PRESET_NAMES = PRESETS.map((preset) => preset.name) as [PresetName, ...PresetName[]];

export type MixStatus = "Standby" | "Silent" | "Solo" | "Live";

/** What the editor and its captures show for a mix. Whether audio is playing is view state,
 * so a capture (which cannot hear) reads the mix as if it were on. */
export function mixerView(data: Mixer, playing = true) {
  const anySolo = CHANNEL_IDS.some((id) => data.soloed[id]);
  const audible = {} as Record<ChannelID, boolean>;
  const levels = {} as ChannelLevels;
  for (const id of CHANNEL_IDS) {
    audible[id] = !data.muted[id] && (anySolo ? data.soloed[id] : true);
    levels[id] = audible[id] ? data.channels[id] : 0;
  }
  const sounding = CHANNEL_IDS.some((id) => levels[id] > 0) && data.master > 0;
  const status: MixStatus = !playing ? "Standby" : !sounding ? "Silent" : anySolo ? "Solo" : "Live";
  const sum = CHANNEL_IDS.reduce((total, id) => total + levels[id], 0);
  /** 0 to 1: how full the meter reads for this mix. */
  const energy = playing ? Math.min(1, (sum / 500) * (data.master / 100)) : 0;
  return { anySolo, audible, levels, sounding, status, energy };
}

function checkLevel(level: number, what: string): number {
  if (!Number.isFinite(level) || level < 0 || level > 100) refuse(`${what} must be between 0 and 100.`);
  return Math.round(level);
}

// Each edit is written once: the page runs it with doc.change, a command with its tx.

/** Move every fader to a preset and clear mute and solo. */
export function applyPreset(tx: Edit, name: string) {
  const preset = PRESETS.find((item) => item.name === name);
  if (!preset) refuse(`Unknown preset "${name}". Choose ${PRESET_NAMES.join(", ")}.`);
  tx.fields.preset.set(preset.name);
  for (const id of CHANNEL_IDS) {
    tx.fields.channels[id].set(preset.levels[id]);
    tx.fields.muted[id].set(false);
    tx.fields.soloed[id].set(false);
  }
}

/** Set one fader; the mix becomes Custom once it no longer matches its preset. */
export function setLevel(tx: Edit, current: Mixer, channel: ChannelID, level: number) {
  const value = checkLevel(level, "A channel level");
  tx.fields.channels[channel].set(value);
  const preset = PRESETS.find((item) => item.name === current.preset);
  if (!preset) return;
  const next = { ...current.channels, [channel]: value };
  if (CHANNEL_IDS.some((id) => Math.round(next[id]) !== preset.levels[id])) tx.fields.preset.set("Custom");
}

export function setMaster(tx: Edit, level: number) {
  tx.fields.master.set(checkLevel(level, "The master volume"));
}

export function setMuted(tx: Edit, channel: ChannelID, muted: boolean) {
  tx.fields.muted[channel].set(muted);
}

export function setSoloed(tx: Edit, channel: ChannelID, soloed: boolean) {
  tx.fields.soloed[channel].set(soloed);
}
