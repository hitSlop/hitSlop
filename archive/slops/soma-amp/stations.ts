import type { Track } from "webamp";

export type Station = {
  id: string;
  title: string;
  genre: string;
  playlist: string;
  fallbackStream: string;
};

export type ResolvedStation = Station & { stream: string; usedFallback: boolean };

export const stations: Station[] = [
  { id: "groovesalad", title: "Groove Salad", genre: "Ambient / Downtempo", playlist: "https://somafm.com/groovesalad130.pls", fallbackStream: "https://ice5.somafm.com/groovesalad-128-aac" },
  { id: "dronezone", title: "Drone Zone", genre: "Deep Ambient", playlist: "https://somafm.com/dronezone130.pls", fallbackStream: "https://ice5.somafm.com/dronezone-128-aac" },
  { id: "spacestation", title: "Space Station Soma", genre: "Space / Ambient", playlist: "https://somafm.com/spacestation130.pls", fallbackStream: "https://ice5.somafm.com/spacestation-128-aac" },
  { id: "secretagent", title: "Secret Agent", genre: "Spy / Lounge", playlist: "https://somafm.com/secretagent130.pls", fallbackStream: "https://ice5.somafm.com/secretagent-128-aac" },
  { id: "beatblender", title: "Beat Blender", genre: "Deep House", playlist: "https://somafm.com/beatblender130.pls", fallbackStream: "https://ice5.somafm.com/beatblender-128-aac" },
  { id: "sonicuniverse", title: "Sonic Universe", genre: "Avant Jazz", playlist: "https://somafm.com/sonicuniverse130.pls", fallbackStream: "https://ice5.somafm.com/sonicuniverse-128-aac" },
  { id: "deepspaceone", title: "Deep Space One", genre: "Deep Electronic", playlist: "https://somafm.com/deepspaceone130.pls", fallbackStream: "https://ice5.somafm.com/deepspaceone-128-aac" },
  { id: "missioncontrol", title: "Mission Control", genre: "Space / NASA", playlist: "https://somafm.com/missioncontrol130.pls", fallbackStream: "https://ice5.somafm.com/missioncontrol-128-aac" },
];

export const streamFromPLS = (text: string): string | null => {
  const candidates = [...text.matchAll(/^File(\d+)=(https:\/\/\S+)\s*$/gim)]
    .sort((left, right) => Number(left[1]) - Number(right[1]));
  return candidates[0]?.[2]?.trim() ?? null;
};

const resolveStation = async (station: Station, signal?: AbortSignal): Promise<ResolvedStation> => {
  const controller = new AbortController();
  const abort = () => controller.abort();
  signal?.addEventListener("abort", abort, { once: true });
  if (signal?.aborted) controller.abort();
  // Every station resolves before the receiver renders; keep a slow directory from stalling startup.
  const timeout = setTimeout(() => controller.abort(), 1_500);
  try {
    const response = await fetch(station.playlist, { cache: "no-store", signal: controller.signal });
    if (!response.ok) throw new Error(String(response.status));
    const stream = streamFromPLS(await response.text());
    if (!stream) throw new Error("No HTTPS stream");
    return { ...station, stream, usedFallback: false };
  } catch {
    return { ...station, stream: station.fallbackStream, usedFallback: true };
  } finally {
    clearTimeout(timeout);
    signal?.removeEventListener("abort", abort);
  }
};

export const resolveStations = (signal?: AbortSignal): Promise<ResolvedStation[]> => Promise.all(stations.map(station => resolveStation(station, signal)));

export const stationTrack = (station: ResolvedStation): Track => ({
  url: station.stream,
  defaultName: station.title,
  // Live radio has no meaningful endpoint. Supplying zero keeps Webamp from
  // probing the endless stream and inventing a multi-hour playlist duration.
  duration: 0,
  metaData: { artist: `SomaFM · ${station.genre}`, title: station.title },
});
