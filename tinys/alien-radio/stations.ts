// The bundled station index. The page replaces it with SomaFM's live catalog
// (https://api.somafm.com/channels.json) when the network answers; offline, exports,
// the icon and commands use this list. Snapshot of the catalog's identity fields taken
// 2026-10-10; listener counts and now-playing lines exist only in the live catalog.

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

/** A bundled entry: its AAC stream playlist, no live listener count or track line. */
function station(id: string, title: string, genre: string, description: string, lastPlaying = ""): Channel {
  return { id, title, description, genre, listeners: "—", lastPlaying, playlists: [{ url: `https://api.somafm.com/${id}130.pls`, format: "aac", quality: "highest" }] };
}

export const fallbackChannels: Channel[] = [
  station("spacestation", "Space Station Soma", "ambient", "Tune in, turn on, space out.", "Awaiting deep-space telemetry"),
  station("missioncontrol", "Mission Control", "ambient|specials", "Celebrating NASA and space explorers everywhere.", "Mission feed standing by"),
  station("deepspaceone", "Deep Space One", "ambient", "Deep ambient electronic and space music.", "Scanning the outer bands"),
  station("dronezone", "Drone Zone", "ambient", "Atmospheric textures with minimal beats.", "Long-range carrier detected"),
  station("beatblender", "Beat Blender", "electronic", "A late night blend of deep-house and downtempo chill."),
  station("brfm", "Black Rock FM", "eclectic", "From the Black Rock Desert playa to the world, year round!"),
  station("bootliquor", "Boot Liquor", "americana", "Americana Roots music for Cowhands, Cowpokes and Cowtippers"),
  station("bossa", "Bossa Beyond", "bossanova|world", "Silky-smooth, laid-back Brazilian-style rhythms of Bossa Nova, Samba and beyond"),
  station("chillits", "Chillits Radio", "chill|live", "Celebrating 25 years of music, chilling and camping"),
  station("cliqhop", "cliqhop idm", "electronic", "Blips'n'beeps backed mostly w/beats. Intelligent Dance Music."),
  station("covers", "Covers", "eclectic", "Just covers. Songs you know by artists you don't. We've got you covered. "),
  station("darkzone", "The Dark Zone", "ambient", "The darker side of deep ambient. Music for staring into the Abyss. "),
  station("defcon", "DEF CON Radio", "electronic|specials", "Music for Hacking. The DEF CON Year-Round Channel."),
  station("digitalis", "Digitalis", "electronic|alternative", "Digitally affected analog rock to calm the agitated heart."),
  station("doomed", "Doomed", "ambient|industrial", "Where every day is Halloween: Dark industrial/ambient music for tortured souls. "),
  station("dz2", "Drone Zone 2", "ambient", "A more eclectic alternative mix of atmospheric textures with minimal beats."),
  station("dubstep", "Dub Step Beyond", "electronic", "Dubstep, Dub and Deep Bass. May damage speakers at high volume."),
  station("fluid", "Fluid", "electronic|hiphop", "Drown in the electronic sound of instrumental hiphop, future soul and liquid trap."),
  station("folkfwd", "Folk Forward", "folk|alternative", "Indie Folk, Alt-folk and the occasional folk classics. "),
  station("groovesalad", "Groove Salad", "ambient|electronic", "A nicely chilled plate of ambient/downtempo beats and grooves."),
  station("groovesalad2", "Groove Salad 2", "ambient|electronic", "A different mix of  nicely chilled ambient/downtempo beats and grooves."),
  station("gsclassic", "Groove Salad Classic", "ambient|electronic", "The classic (early 2000s) version of a nicely chilled plate of ambient/downtempo beats and grooves."),
  station("reggae", "Heavyweight Reggae", "reggae", "Reggae, Ska, Rocksteady classic and deep tracks."),
  station("illstreet", "Illinois Street Lounge", "lounge", "Classic bachelor pad, playful exotica and vintage music of tomorrow."),
  station("insound", "The In-Sound", "oldies", "60s/70s Hipster Euro Pop where psychedelic melodies meets groovy vibes. "),
  station("indiepop", "Indie Pop Rocks!", "alternative|rock", "New and classic favorite indie pop tracks."),
  station("seventies", "Left Coast 70s", "70s|rock", "Mellow album rock from the Seventies. Yacht not required."),
  station("lush", "Lush", "electronic", "Sensuous and mellow female vocals, many with an electronic influence."),
  station("metal", "Metal Detector", "metal", "From black to doom, prog to sludge, thrash to post, stoner to crossover, punk to industrial."),
  station("n5md", "n5MD Radio", "specials", "Emotional Experiments in Music: Ambient, modern composition, post-rock, & experimental electronic music"),
  station("poptron", "PopTron", "alternative", "Electropop and indie dance rock with sparkle and pop."),
  station("secretagent", "Secret Agent", "lounge", "The soundtrack for your stylish, mysterious, dangerous life. For Spies and PIs too!"),
  station("7soul", "Seven Inch Soul", "oldies", "Vintage soul tracks from the original 45 RPM vinyl."),
  station("sf1033", "SF 10-33", "ambient|news", "Ambient music mixed with the sounds of San Francisco public safety radio traffic."),
  station("sfinsf", "SF in SF", "spoken", "Author readings and discussions from the science fiction, fantasy, horror, and genre literary fields."),
  station("scanner", "SF Police Scanner", "live|news", "San Francisco Public Safety Scanner Feed"),
  station("live", "SomaFM Live", "live|specials", "Special Live Events and rebroadcasts of past live events"),
  station("specials", "SomaFM Specials", "specials", "Now featuring Afternoon Jazz, Wavepool, DubX, The Surf Report & More!"),
  station("sonicuniverse", "Sonic Universe", "jazz", "Transcending the world of jazz with eclectic, avant-garde takes on tradition."),
  station("suburbsofgoa", "Suburbs of Goa", "world", "Desi-influenced Asian world beats and beyond."),
  station("synphaera", "Synphaera Radio", "ambient|electronic", "Featuring the music from an independent record label focused on modern electronic ambient and space music."),
  station("thistle", "ThistleRadio", "celtic|world", "Exploring music from Celtic roots and branches"),
  station("tikitime", "Tiki Time", "tiki|world", "Classic Tiki music and Vintage island rhythms to sip cocktails by."),
  station("thetrip", "The Trip", "electronic", "Progressive house / trance. Tip top tunes."),
  station("u80s", "Underground 80s", "alternative|electronic", "Early 80s UK Synthpop and a bit of New Wave."),
  station("vaporwaves", "Vaporwaves", "electronic", "All Vaporwave. All the time."),
];

export const stationIds = fallbackChannels.map(channel => channel.id) as [string, ...string[]];

/** AAC first, then the highest quality, as SomaFM lists them. */
export function preferredPlaylist(channel: Channel): Playlist | undefined {
  const score = (item: Playlist) => (item.format === "aac" ? 30 : item.format === "aacp" ? 20 : 10) + (item.quality === "highest" ? 3 : item.quality === "high" ? 2 : 1);
  return [...channel.playlists].sort((a, b) => score(b) - score(a))[0];
}
