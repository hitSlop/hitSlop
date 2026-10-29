import type { Track } from "webamp";
import { stations, type Station, type ResolvedStation } from "./station-list";
export * from "./station-list";

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
