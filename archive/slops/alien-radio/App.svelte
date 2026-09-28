<script lang="ts">
  import { Slop, useDocument } from "@hitslop/document/svelte";
  import { onMount } from "svelte";
  import { Button, Popover, Slider, Toggle } from "bits-ui";
  import schema from "./schema";

  const chromeUrl = "/assets/alien-radio-chrome.png";

  type Playlist = { url: string; format: string; quality: string };
  type Channel = {
    id: string;
    title: string;
    description: string;
    genre: string;
    listeners: string;
    lastPlaying: string;
    playlists: Playlist[];
  };

  const fallbackChannels: Channel[] = [
    { id: "spacestation", title: "Space Station Soma", description: "Tune in, turn on, space out.", genre: "ambient", listeners: "—", lastPlaying: "Awaiting deep-space telemetry", playlists: [{ url: "https://api.somafm.com/spacestation130.pls", format: "aac", quality: "highest" }] },
    { id: "missioncontrol", title: "Mission Control", description: "Celebrating NASA and space explorers everywhere.", genre: "ambient|specials", listeners: "—", lastPlaying: "Mission feed standing by", playlists: [{ url: "https://api.somafm.com/missioncontrol130.pls", format: "aac", quality: "highest" }] },
    { id: "deepspaceone", title: "Deep Space One", description: "Deep ambient electronic and space music.", genre: "ambient", listeners: "—", lastPlaying: "Scanning the outer bands", playlists: [{ url: "https://api.somafm.com/deepspaceone130.pls", format: "aac", quality: "highest" }] },
    { id: "dronezone", title: "Drone Zone", description: "Atmospheric textures with minimal beats.", genre: "ambient", listeners: "—", lastPlaying: "Long-range carrier detected", playlists: [{ url: "https://api.somafm.com/dronezone130.pls", format: "aac", quality: "highest" }] },
  ];

  const doc = useDocument(schema);
  let channels = $state<Channel[]>(fallbackChannels);
  let browserOpen = $state(false);
  let query = $state("");
  let loading = $state(false);
  let playing = $state(false);
  let online = $state(false);
  let status = $state("STANDBY");
  let error = $state<string | null>(null);
  let canvas: HTMLCanvasElement;
  let animationFrame = 0;
  const audio = new Audio();
  audio.crossOrigin = "anonymous";
  audio.preload = "none";
  const streamCache = new Map<string, string>();
  let audioContext: AudioContext | null = null;
  let analyser: AnalyserNode | null = null;
  let source: MediaElementAudioSourceNode | null = null;

  const selected = $derived(channels.find(channel => channel.id === doc.current.selectedChannelId) ?? channels[0] ?? fallbackChannels[0]!);
  const isFavorite = $derived(doc.current.favoriteChannelIds.includes(selected.id));
  const filtered = $derived(channels.filter(channel => `${channel.title} ${channel.description} ${channel.genre}`.toLowerCase().includes(query.trim().toLowerCase())));

  function preferredPlaylist(channel: Channel): Playlist | undefined {
    return [...channel.playlists].sort((a, b) => {
      const score = (item: Playlist) => (item.format === "aac" ? 30 : item.format === "aacp" ? 20 : 10) + (item.quality === "highest" ? 3 : item.quality === "high" ? 2 : 1);
      return score(b) - score(a);
    })[0];
  }

  async function resolveStream(channel: Channel): Promise<string> {
    const cached = streamCache.get(channel.id);
    if (cached) return cached;
    const playlist = preferredPlaylist(channel);
    if (!playlist) throw new Error("No compatible stream is listed for this channel.");
    const response = await fetch(playlist.url, { cache: "no-store" });
    if (!response.ok) throw new Error(`Playlist request failed (${response.status}).`);
    const match = (await response.text()).match(/^File\d+=(.+)$/im);
    if (!match?.[1]) throw new Error("The station playlist did not contain a stream URL.");
    const stream = match[1].trim();
    streamCache.set(channel.id, stream);
    return stream;
  }

  async function prepareAudioGraph(): Promise<void> {
    audioContext ??= new AudioContext();
    analyser ??= audioContext.createAnalyser();
    analyser.fftSize = 64;
    analyser.smoothingTimeConstant = 0.82;
    if (!source) {
      source = audioContext.createMediaElementSource(audio);
      source.connect(analyser);
      analyser.connect(audioContext.destination);
    }
    await audioContext.resume();
  }

  async function playSelected(): Promise<void> {
    loading = true;
    error = null;
    status = "TUNING";
    try {
      const stream = await resolveStream(selected);
      if (audio.src !== stream) audio.src = stream;
      await prepareAudioGraph();
      await audio.play();
    } catch (cause) {
      playing = false;
      status = "NO SIGNAL";
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      loading = false;
    }
  }

  function togglePlayback(): void {
    if (audio.paused) void playSelected();
    else audio.pause();
  }

  function choose(channel: Channel): void {
    const wasPlaying = !audio.paused;
    if (channel.id !== doc.current.selectedChannelId) doc.fields.selectedChannelId.set(channel.id);
    browserOpen = false;
    query = "";
    error = null;
    if (wasPlaying) setTimeout(() => void playSelected(), 0);
    else void resolveStream(channel).catch(() => undefined);
  }

  function cycle(direction: -1 | 1): void {
    const favorites = channels.filter(channel => doc.current.favoriteChannelIds.includes(channel.id));
    const pool = favorites.length ? favorites : channels;
    const index = Math.max(0, pool.findIndex(channel => channel.id === selected.id));
    choose(pool[(index + direction + pool.length) % pool.length]!);
  }

  function setFavorite(pressed: boolean): void {
    const ids = doc.current.favoriteChannelIds;
    if (pressed && !ids.includes(selected.id)) doc.fields.favoriteChannelIds.insert(selected.id);
    else if (!pressed) {
      const index = ids.indexOf(selected.id);
      if (index >= 0) doc.fields.favoriteChannelIds.remove(index);
    }
  }

  function previewVolume(value: number): void {
    doc.fields.volume.preview(value);
    if (value > 0 && doc.current.muted) doc.fields.muted.set(false);
  }

  function commitVolume(value: number): void {
    doc.fields.volume.set(value);
    if (value > 0 && doc.current.muted) doc.fields.muted.set(false);
  }

  async function refreshChannels(): Promise<void> {
    try {
      const response = await fetch("https://api.somafm.com/channels.json", { cache: "no-store" });
      if (!response.ok) throw new Error(String(response.status));
      const payload = await response.json() as { channels?: Channel[] };
      if (!payload.channels?.length) throw new Error("empty catalog");
      channels = payload.channels;
      online = true;
    } catch {
      online = false;
    }
  }

  function drawSpectrum(): void {
    if (!canvas) return;
    const context = canvas.getContext("2d");
    if (!context) return;
    const width = canvas.width = Math.round(canvas.clientWidth * devicePixelRatio);
    const height = canvas.height = Math.round(canvas.clientHeight * devicePixelRatio);
    context.clearRect(0, 0, width, height);
    const values = new Uint8Array(analyser?.frequencyBinCount ?? 32);
    if (analyser && playing) analyser.getByteFrequencyData(values);
    const bars = 20;
    const gap = 3 * devicePixelRatio;
    const barWidth = (width - gap * (bars - 1)) / bars;
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    for (let index = 0; index < bars; index += 1) {
      const live = values[Math.floor(index / bars * values.length)] ?? 0;
      const idle = reduced ? 18 : 18 + Math.sin(Date.now() / 620 + index * 0.72) * 7;
      const amount = playing ? Math.max(10, live) : idle;
      const barHeight = Math.max(2, amount / 255 * height);
      context.fillStyle = index > 15 ? "#ad7cff" : "#caff42";
      context.fillRect(index * (barWidth + gap), height - barHeight, barWidth, barHeight);
    }
    animationFrame = requestAnimationFrame(drawSpectrum);
  }

  $effect(() => {
    audio.volume = Math.max(0, Math.min(1, doc.current.volume));
    audio.muted = doc.current.muted;
  });

  onMount(() => {
    const onPlaying = () => { playing = true; loading = false; status = "RECEIVING"; error = null; };
    const onPause = () => { playing = false; status = "PAUSED"; };
    const onWaiting = () => { loading = true; status = "BUFFERING"; };
    const onError = () => { playing = false; loading = false; status = "NO SIGNAL"; error = "The stream dropped. Check the connection and retry."; };
    audio.addEventListener("playing", onPlaying);
    audio.addEventListener("pause", onPause);
    audio.addEventListener("waiting", onWaiting);
    audio.addEventListener("error", onError);
    void refreshChannels();
    const refresh = setInterval(refreshChannels, 15_000);
    animationFrame = requestAnimationFrame(drawSpectrum);
    return () => {
      clearInterval(refresh);
      cancelAnimationFrame(animationFrame);
      audio.pause();
      audio.removeEventListener("playing", onPlaying);
      audio.removeEventListener("pause", onPause);
      audio.removeEventListener("waiting", onWaiting);
      audio.removeEventListener("error", onError);
      void audioContext?.close();
    };
  });
</script>

<Slop>
  <main class="radio" aria-label="Alien Radio SomaFM receiver" data-slop-selection="none">
    <img class="chrome" src={chromeUrl} alt="" draggable="false" />
    <header class="identity">
      <span class="lamp" data-online={online}></span>
      <strong>ALIEN RADIO</strong>
      <small>INTERSTELLAR RECEIVER / AR-01</small>
    </header>
    <section class="display" aria-live="polite">
      <div class="topline">
        <span>{status}</span>
        <span>{online ? "NET LINK" : "LOCAL INDEX"}</span>
        <span>{selected.listeners === "—" ? "—" : `${selected.listeners} EARTHLINGS`}</span>
      </div>
      <Popover.Root bind:open={browserOpen}>
        <Popover.Trigger class="station" data-slop-export="hide">
          <span>{selected.genre.split("|")[0] || "broadcast"}</span>
          <strong>{selected.title}</strong>
          <i>CHOOSE ↗</i>
        </Popover.Trigger>
        <Popover.ContentStatic class="browser" preventScroll={false} data-slop-export="hide" aria-label="Find frequency">
          <label>
            <span>FIND FREQUENCY</span>
            <input bind:value={query} placeholder="station, mood, genre" />
          </label>
          <div class="channels">
            {#each filtered.slice(0, 7) as channel}
              <Button.Root type="button" class="channel" data-selected={channel.id === selected.id} onclick={() => choose(channel)}>
                <strong>{channel.title}</strong><span>{channel.genre.replaceAll("|", " / ")}</span>
              </Button.Root>
            {:else}
              <p>No signal on that band.</p>
            {/each}
          </div>
        </Popover.ContentStatic>
      </Popover.Root>
      <p class="track">{selected.lastPlaying || selected.description}</p>
      <canvas bind:this={canvas} class="spectrum" aria-label="Live audio spectrum"></canvas>
    </section>
    <section class="transport" aria-label="Playback controls" data-slop-export="hide">
      <Button.Root type="button" class="round" onclick={() => cycle(-1)} aria-label="Previous favorite">◀</Button.Root>
      <Button.Root type="button" class="round play" onclick={togglePlayback} disabled={loading} aria-label={playing ? "Pause" : "Play"}>{loading ? "···" : playing ? "Ⅱ" : "▶"}</Button.Root>
      <Button.Root type="button" class="round" onclick={() => cycle(1)} aria-label="Next favorite">▶</Button.Root>
      <Toggle.Root class="favorite" pressed={isFavorite} onPressedChange={setFavorite} aria-label={isFavorite ? "Remove favorite" : "Add favorite"}>★</Toggle.Root>
    </section>
    <section class="gain" aria-label="Volume">
      <button data-slop-export="hide" onclick={() => doc.fields.muted.set(!doc.current.muted)}>{doc.current.muted ? "MUTED" : "GAIN"}</button>
      <Slider.Root type="single" value={doc.current.volume} onValueChange={previewVolume} onValueCommit={commitVolume} min={0} max={1} step={0.01} class="slider" aria-label="Gain volume">
        {#snippet children({ thumbs })}
          <span class="sliderTrack"><Slider.Range class="sliderRange" /></span>
          {#each thumbs as index}
            <Slider.Thumb {index} class="sliderThumb" aria-label="Volume" />
          {/each}
        {/snippet}
      </Slider.Root>
      <output>{Math.round(doc.current.volume * 100)}</output>
    </section>
    {#if error}
      <button class="error" data-slop-export="hide" onclick={() => void playSelected()}>{error} <strong>RETRY</strong></button>
    {/if}
    <a class="soma" href="https://somafm.com/support/" target="_blank" rel="noreferrer">Powered by listener-supported SomaFM · Support the signal ↗</a>
  </main>

  {#snippet exportView()}
    <article class="radio" aria-label="Exported Alien Radio">
      <img class="chrome" src={chromeUrl} alt="" />
      <header class="identity"><span class="lamp"></span><strong>ALIEN RADIO</strong><small>INTERSTELLAR RECEIVER / AR-01</small></header>
      <section class="display">
        <div class="topline"><span>{status}</span><span>LOCAL INDEX</span><span>—</span></div>
        <div class="station"><span>broadcast</span><strong>{selected.title}</strong></div>
        <p class="track">{selected.lastPlaying || selected.description}</p>
      </section>
    </article>
  {/snippet}

  {#snippet icon()}
    <div class="iconSurface" aria-hidden="true">
      <article class="iconTile"><div class="iconScreen"><i></i><i></i><i></i><i></i><i></i><span>◉</span></div></article>
    </div>
  {/snippet}
</Slop>
