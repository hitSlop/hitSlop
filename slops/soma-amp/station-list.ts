
export const stationIds = ["groovesalad", "dronezone", "spacestation", "secretagent", "beatblender", "sonicuniverse", "deepspaceone", "missioncontrol"] as const;
export type StationId = (typeof stationIds)[number];

export type Station = {
  id: StationId;
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
