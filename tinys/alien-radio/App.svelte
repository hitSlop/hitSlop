<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  import { getDocument } from "@hitslop/tinys/svelte";
  import { onMount } from "svelte";
  import { Button, Popover, Slider, Toggle } from "bits-ui";
  import schema from "./schema";
  import chromeUrl from "./assets/alien-radio-chrome.png";
  import { radioView, selectChannel, setFavorite, setMuted, setVolume, stepChannel } from "./radio";
  import { fallbackChannels, preferredPlaylist, type Channel } from "./stations";

  const doc = getDocument(schema);

  // The catalog, playback and the audio graph are view state: the document stores only
  // the tuned station, favorites and gain. A reopened receiver waits for play.
  let channels = $state<Channel[]>(fallbackChannels);
  let catalog = $state<"scanning" | "online" | "offline">("scanning");
  let status = $state("STANDBY");
  let browserOpen = $state(false);
  let query = $state("");
  let loading = $state(false);
  let playing = $state(false);
  let error = $state<string | null>(null);
  let canvas: HTMLCanvasElement;
  let animationFrame = 0;
  const streamCache = new Map<string, string>();
  let audio: HTMLAudioElement | null = null;
  let audioContext: AudioContext | null = null;
  let analyser: AnalyserNode | null = null;
  let source: MediaElementAudioSourceNode | null = null;

  const view = $derived(radioView(doc.current, channels));
  const selected = $derived(view.selected);
  const filtered = $derived(channels.filter(channel => `${channel.title} ${channel.description} ${channel.genre}`.toLowerCase().includes(query.trim().toLowerCase())));

  const onPlaying = () => { playing = true; loading = false; status = "RECEIVING"; error = null; };
  const onPause = () => { playing = false; status = "PAUSED"; };
  const onWaiting = () => { loading = true; status = "BUFFERING"; };
  const onError = () => { playing = false; loading = false; status = "NO SIGNAL"; error = "The stream dropped. Check the connection and retry."; };

  function applyGain(): void {
    if (!audio) return;
    audio.volume = Math.max(0, Math.min(1, doc.current.volume));
    audio.muted = doc.current.muted;
  }

  /** The audio element and its WebAudio graph are created on the first play press. */
  function ensureAudio(): HTMLAudioElement {
    if (audio) return audio;
    audio = new Audio();
    audio.crossOrigin = "anonymous";
    audio.preload = "none";
    audio.addEventListener("playing", onPlaying);
    audio.addEventListener("pause", onPause);
    audio.addEventListener("waiting", onWaiting);
    audio.addEventListener("error", onError);
    applyGain();
    return audio;
  }

  async function resolveStream(channel: Channel): Promise<string> {
    const cached = streamCache.get(channel.id);
    if (cached) return cached;
    const playlist = preferredPlaylist(channel);
    if (!playlist) throw new Error("No compatible stream is listed for this channel.");
    let response: Response;
    try {
      response = await fetch(playlist.url, { cache: "no-store" });
    } catch {
      throw new Error("Can't reach SomaFM. Check the connection and retry.");
    }
    if (!response.ok) throw new Error(`Playlist request failed (${response.status}).`);
    const match = (await response.text()).match(/^File\d+=(.+)$/im);
    if (!match?.[1]) throw new Error("The station playlist did not contain a stream URL.");
    const stream = match[1].trim();
    streamCache.set(channel.id, stream);
    return stream;
  }

  async function prepareAudioGraph(element: HTMLAudioElement): Promise<void> {
    audioContext ??= new AudioContext();
    analyser ??= audioContext.createAnalyser();
    analyser.fftSize = 64;
    analyser.smoothingTimeConstant = 0.82;
    if (!source) {
      source = audioContext.createMediaElementSource(element);
      source.connect(analyser);
      analyser.connect(audioContext.destination);
    }
    await audioContext.resume();
  }

  async function play(channel: Channel): Promise<void> {
    loading = true;
    error = null;
    status = "TUNING";
    try {
      const element = ensureAudio();
      const stream = await resolveStream(channel);
      if (element.src !== stream) element.src = stream;
      await prepareAudioGraph(element);
      await element.play();
    } catch (cause) {
      playing = false;
      status = "NO SIGNAL";
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      loading = false;
    }
  }

  function togglePlayback(): void {
    if (!audio || audio.paused) void play(selected);
    else audio.pause();
  }

  function choose(channel: Channel): void {
    const wasPlaying = !!audio && !audio.paused;
    void doc.change(tx => selectChannel(tx, doc.current, channel.id));
    browserOpen = false;
    query = "";
    error = null;
    if (wasPlaying) void play(channel);
    else void resolveStream(channel).catch(() => undefined);
  }

  function cycle(direction: -1 | 1): void {
    const next = stepChannel(doc.current, channels, direction);
    if (next) choose(next);
  }

  function toggleFavorite(pressed: boolean): void {
    void doc.change(tx => setFavorite(tx, doc.current, selected.id, pressed));
  }

  function previewVolume(value: number): void {
    doc.fields.volume.preview(value);
    if (value > 0 && doc.current.muted) void doc.change(tx => setMuted(tx, false));
  }

  function commitVolume(value: number): void {
    void doc.change(tx => setVolume(tx, doc.current, value));
  }

  async function refreshChannels(): Promise<void> {
    if (document.hidden) return;
    try {
      const response = await fetch("https://api.somafm.com/channels.json", { cache: "no-store" });
      if (!response.ok) throw new Error(String(response.status));
      const payload = await response.json() as { channels?: Channel[] };
      if (!payload.channels?.length) throw new Error("empty catalog");
      channels = payload.channels;
      catalog = "online";
    } catch {
      catalog = "offline";
    }
  }

  function drawSpectrum(): void {
    animationFrame = requestAnimationFrame(drawSpectrum);
    if (!canvas) return;
    const context = canvas.getContext("2d");
    if (!context) return;
    const width = canvas.width = Math.round(canvas.clientWidth * devicePixelRatio);
    const height = canvas.height = Math.round(canvas.clientHeight * devicePixelRatio);
    context.clearRect(0, 0, width, height);
    const values = new Uint8Array(analyser?.frequencyBinCount ?? 32);
    if (analyser && playing) analyser.getByteFrequencyData(values);
    const lime = getComputedStyle(canvas).getPropertyValue("--tiny-lime").trim() || "#caff42";
    const bars = 20;
    const gap = 3 * devicePixelRatio;
    const barWidth = (width - gap * (bars - 1)) / bars;
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    for (let index = 0; index < bars; index += 1) {
      const live = values[Math.floor(index / bars * values.length)] ?? 0;
      const idle = reduced ? 18 : 18 + Math.sin(Date.now() / 620 + index * 0.72) * 7;
      const amount = playing ? Math.max(10, live) : idle;
      const barHeight = Math.max(2, amount / 255 * height);
      context.fillStyle = index > 15 ? "#ad7cff" : lime;
      context.fillRect(index * (barWidth + gap), height - barHeight, barWidth, barHeight);
    }
  }

  $effect(() => {
    void doc.current.volume;
    void doc.current.muted;
    applyGain();
  });

  onMount(() => {
    void refreshChannels();
    const refresh = setInterval(refreshChannels, 15_000);
    animationFrame = requestAnimationFrame(drawSpectrum);
    return () => {
      clearInterval(refresh);
      cancelAnimationFrame(animationFrame);
      if (audio) {
        audio.pause();
        audio.removeEventListener("playing", onPlaying);
        audio.removeEventListener("pause", onPause);
        audio.removeEventListener("waiting", onWaiting);
        audio.removeEventListener("error", onError);
        audio.removeAttribute("src");
        audio.load();
      }
      void audioContext?.close();
    };
  });
</script>

<main class="radio" aria-label="Alien Radio SomaFM receiver">
  <img class="chrome" src={chromeUrl} alt="" draggable="false" />
  <header class="identity">
    <span class="lamp" data-online={catalog === "online"}></span>
    <strong>ALIEN RADIO</strong>
    <small>INTERSTELLAR RECEIVER / AR-01</small>
  </header>
  <section class="display" aria-live="polite">
    <div class="topline">
      <span>{status}</span>
      <span>{catalog === "online" ? "NET LINK" : catalog === "offline" ? "OFFLINE · LOCAL INDEX" : "SCANNING"}</span>
      <span>{selected.listeners === "—" ? "—" : `${selected.listeners} EARTHLINGS`}</span>
    </div>
    <Popover.Root bind:open={browserOpen}>
      <Popover.Trigger class="station" data-tiny-export="hide">
        <span>{selected.genre.split("|")[0] || "broadcast"}</span>
        <strong>{selected.title}</strong>
        <i>CHOOSE ↗</i>
      </Popover.Trigger>
      <Popover.ContentStatic class="browser" preventScroll={false} data-tiny-export="hide" aria-label="Find frequency">
        <label>
          <span>{catalog === "offline" ? "FIND FREQUENCY · SOMAFM UNREACHABLE, BUNDLED INDEX" : "FIND FREQUENCY"}</span>
          <input bind:value={query} placeholder="station, mood, genre" />
        </label>
        <div class="channels">
          {#each filtered.slice(0, 7) as channel (channel.id)}
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
  <section class="transport" aria-label="Playback controls" data-tiny-export="hide">
    <Button.Root type="button" class="round" onclick={() => cycle(-1)} aria-label="Previous favorite">◀</Button.Root>
    <Button.Root type="button" class="round play" onclick={togglePlayback} disabled={loading} aria-label={playing ? "Pause" : "Play"}>{loading ? "···" : playing ? "Ⅱ" : "▶"}</Button.Root>
    <Button.Root type="button" class="round" onclick={() => cycle(1)} aria-label="Next favorite">▶</Button.Root>
    <Toggle.Root class="favorite" pressed={view.isFavorite} onPressedChange={toggleFavorite} aria-label={view.isFavorite ? "Remove favorite" : "Add favorite"}>★</Toggle.Root>
  </section>
  <section class="gain" aria-label="Volume">
    <button data-tiny-export="hide" onclick={() => void doc.change(tx => setMuted(tx, !doc.current.muted))}>{doc.current.muted ? "MUTED" : "GAIN"}</button>
    <Slider.Root type="single" value={doc.current.volume} onValueChange={previewVolume} onValueCommit={commitVolume} min={0} max={1} step={0.01} class="slider" aria-label="Gain volume">
      {#snippet children({ thumbs })}
        <span class="sliderTrack"><Slider.Range class="sliderRange" /></span>
        {#each thumbs as index (index)}
          <Slider.Thumb {index} class="sliderThumb" aria-label="Volume" />
        {/each}
      {/snippet}
    </Slider.Root>
    <output>{view.gain}</output>
  </section>
  {#if error}
    <button class="error" data-tiny-export="hide" onclick={() => void play(selected)}>{error} <strong>RETRY</strong></button>
  {/if}
  <a class="soma" href="https://somafm.com/support/" target="_blank" rel="noreferrer">Powered by listener-supported SomaFM · Support the signal ↗</a>
</main>
