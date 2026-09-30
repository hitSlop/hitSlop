
export type Playlist = { url: string; format: string; quality: string };
export type Channel = {
    id: string;
    title: string;
    description: string;
    genre: string;
    listeners: string;
    lastPlaying: string;
    playlists: Playlist[];
  };
export const fallbackChannels: Channel[] = [
    { id: "spacestation", title: "Space Station Soma", description: "Tune in, turn on, space out.", genre: "ambient", listeners: "—", lastPlaying: "Awaiting deep-space telemetry", playlists: [{ url: "https://api.somafm.com/spacestation130.pls", format: "aac", quality: "highest" }] },
    { id: "missioncontrol", title: "Mission Control", description: "Celebrating NASA and space explorers everywhere.", genre: "ambient|specials", listeners: "—", lastPlaying: "Mission feed standing by", playlists: [{ url: "https://api.somafm.com/missioncontrol130.pls", format: "aac", quality: "highest" }] },
    { id: "deepspaceone", title: "Deep Space One", description: "Deep ambient electronic and space music.", genre: "ambient", listeners: "—", lastPlaying: "Scanning the outer bands", playlists: [{ url: "https://api.somafm.com/deepspaceone130.pls", format: "aac", quality: "highest" }] },
    { id: "dronezone", title: "Drone Zone", description: "Atmospheric textures with minimal beats.", genre: "ambient", listeners: "—", lastPlaying: "Long-range carrier detected", playlists: [{ url: "https://api.somafm.com/dronezone130.pls", format: "aac", quality: "highest" }] },
  ];
export const ui = $state({
  status: "STANDBY",
  channels: fallbackChannels as Channel[],
});
