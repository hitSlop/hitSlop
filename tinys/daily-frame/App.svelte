<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  import { onMount, tick } from "svelte";
  import { prefersReducedMotion } from "svelte/motion";
  import { AlertDialog, Select, Slider, Toggle, ToggleGroup } from "bits-ui";
  import { attachments, EditableText, getDocument, notify } from "@hitslop/tinys/svelte";
  import Play from "@lucide/svelte/icons/play";
  import Pause from "@lucide/svelte/icons/pause";
  import Square from "@lucide/svelte/icons/square";
  import X from "@lucide/svelte/icons/x";
  import Trash from "@lucide/svelte/icons/trash-2";
  import ImageUp from "@lucide/svelte/icons/image-up";
  import Layers from "@lucide/svelte/icons/layers";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import CameraOff from "@lucide/svelte/icons/camera-off";
  import schema, { maxFrameBytes } from "./schema";
  import { dayKey, dayLabel, frameView, keepFrame, monthEarlier, nearFrame, removeFrame, shortLabel, sizeLabel, stamp } from "./frames";
  import { explainCameraError, fileToJpeg, listCameras, openCamera, stopStream, toJpeg } from "./camera";
  import Photo from "./Photo.svelte";

  const doc = getDocument(schema);

  type CameraState =
    | { phase: "starting" }
    | { phase: "live" }
    | { phase: "saving"; still: string }
    | { phase: "error"; title: string; detail: string };

  // View state only: none of this is saved, and the export cannot see it.
  let now = $state(new Date());
  let visible = $state(document.visibilityState === "visible");
  let mode = $state<"view" | "camera" | "play">("view");
  let picked = $state<string | null>(null);
  let camera = $state<CameraState>({ phase: "starting" });
  let cameras = $state<{ value: string; label: string }[]>([]);
  let cameraId = $state("");
  let ghost = $state(true);
  let flash = $state(false);
  let importing = $state(false);
  let removing = $state(false);
  let index = $state(0);
  let running = $state(false);
  let fps = $state("6");
  let video = $state<HTMLVideoElement>();
  let fileInput = $state<HTMLInputElement>();
  let strip = $state<HTMLElement>();
  // The live stream is transient and never reactive: it lives only while the viewfinder is open.
  let stream: MediaStream | null = null;
  let attempt = 0;

  const today = $derived(dayKey(now));
  const view = $derived(frameView(doc.current, today));
  const stageDay = $derived(picked && view.byDate.has(picked) ? picked : today);
  const stageFrame = $derived(view.byDate.get(stageDay) ?? null);
  const earlierDay = $derived(monthEarlier(stageDay));
  const earlier = $derived(nearFrame(view, earlierDay));
  const slots = $derived(view.todayFrame ? view.stack.map(shown => shown.date) : [...view.stack.map(shown => shown.date), today]);
  const playing = $derived(view.stack[Math.min(index, view.stack.length - 1)] ?? null);
  const hasTodayPhoto = $derived(doc.current.frames.some(frame => frame.date === today));
  const ghostUrl = $derived(view.previous?.photo ? attachments.url(view.previous.photo) : "");

  // ---- Camera --------------------------------------------------------------------

  async function startCamera(deviceId = cameraId || undefined) {
    stopPlay();
    stopCamera();
    const mine = attempt;
    mode = "camera";
    camera = { phase: "starting" };
    try {
      const opened = await openCamera(deviceId);
      if (mine !== attempt || mode !== "camera") { stopStream(opened); return; }
      stream = opened;
      await tick();
      if (!video) throw new Error("The viewfinder isn't ready.");
      video.srcObject = opened;
      await video.play();
      if (mine !== attempt) return;
      camera = { phase: "live" };
      const track = opened.getVideoTracks()[0];
      if (track) track.onended = () => {
        if (mine !== attempt) return;
        stopCamera();
        camera = { phase: "error", title: "The camera stopped", detail: "It was disconnected or another app took it. Try again when it's back." };
      };
      cameraId = track?.getSettings().deviceId ?? "";
      cameras = await listCameras();
    } catch (error) {
      if (mine !== attempt) return;
      stopCamera();
      camera = { phase: "error", ...explainCameraError(error) };
    }
  }
  /** Stops every track. Safe to call at any time. */
  function stopCamera() {
    attempt++;
    stopStream(stream);
    stream = null;
    if (video) video.srcObject = null;
  }
  function closeCamera() {
    stopCamera();
    if (mode === "camera") mode = "view";
  }

  async function shutter() {
    if (camera.phase !== "live" || !video) return;
    now = new Date();
    const date = dayKey(now);
    let shot;
    try {
      shot = await toJpeg(video, video.videoWidth, video.videoHeight);
    } catch (error) {
      stopCamera();
      camera = { phase: "error", title: "The photo wasn't taken", detail: error instanceof Error ? error.message : "Try again." };
      return;
    }
    if (!prefersReducedMotion.current) { flash = true; setTimeout(() => (flash = false), 280); }
    const still = URL.createObjectURL(shot.blob);
    camera = { phase: "saving", still };
    stopCamera(); // the camera goes off as soon as the picture exists
    const problem = await keep(shot.blob, date);
    URL.revokeObjectURL(still);
    if (problem) { camera = { phase: "error", title: "The frame wasn't kept", detail: problem }; return; }
    mode = "view";
    picked = date;
  }

  /** Imports a finished JPEG as `date`'s frame. Returns a message when it was not kept. */
  async function keep(blob: Blob, date: string) {
    if (blob.size > maxFrameBytes) return `This picture is ${sizeLabel(blob.size)}; one frame can be at most 10 MB.`;
    const retake = doc.current.frames.some(frame => frame.date === date);
    try {
      await attachments.import<typeof schema.descriptor>(new File([blob], `frame-${date}.jpg`, { type: "image/jpeg" }), (tx, ref) => {
        keepFrame(tx, doc.current, date, ref);
      });
    } catch (error) {
      return error instanceof Error ? error.message : "The photo couldn't be stored in this document.";
    }
    notify(retake ? `Retook ${shortLabel(date)}. The old photo leaves the file when it closes.` : `Kept ${date === today ? "today's" : shortLabel(date)} frame · ${sizeLabel(blob.size)}.`);
    return null;
  }

  async function fileChosen(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = "";
    if (!file) return;
    if (!file.type.startsWith("image/")) { notify("Choose a photo file."); return; }
    importing = true;
    now = new Date();
    const date = dayKey(now);
    try {
      const shot = await fileToJpeg(file);
      const problem = await keep(shot.blob, date);
      if (problem) notify(problem);
      else { closeCamera(); picked = date; }
    } catch {
      notify("That file couldn't be opened as a picture. Try a JPEG or PNG.");
    } finally {
      importing = false;
    }
  }

  // ---- Flipbook ------------------------------------------------------------------

  function startPlay() {
    closeCamera();
    const at = view.stack.findIndex(shown => shown.date === stageDay);
    index = at > 0 && at < view.stack.length - 1 ? at : 0;
    mode = "play";
    running = true;
  }
  function stopPlay() {
    if (mode !== "play") return;
    running = false;
    mode = "view";
    if (playing) picked = playing.date;
  }
  $effect(() => {
    if (mode !== "play" || !running || !visible) return;
    const length = view.stack.length;
    const timer = setInterval(() => (index = (index + 1) % length), 1000 / Number(fps));
    return () => clearInterval(timer);
  });

  // ---- Stack ---------------------------------------------------------------------

  /** The first slot of each month in the strip carries the month's name. */
  function newMonth(day: string) {
    const before = slots[slots.indexOf(day) - 1];
    return !before || before.slice(0, 7) !== day.slice(0, 7);
  }
  function pick(day: string) {
    if (mode === "play") stopPlay();
    picked = day === today ? null : day;
  }
  function stripKeys(event: KeyboardEvent) {
    const at = slots.indexOf(stageDay);
    const next = event.key === "ArrowLeft" ? at - 1 : event.key === "ArrowRight" ? at + 1
      : event.key === "Home" ? 0 : event.key === "End" ? slots.length - 1 : null;
    if (next === null) return;
    event.preventDefault();
    const day = slots[Math.max(0, Math.min(slots.length - 1, next))];
    if (!day) return;
    pick(day);
    void tick().then(() => strip?.querySelector<HTMLElement>(`[data-day="${day}"]`)?.focus());
  }
  $effect(() => {
    const day = mode === "play" && playing ? playing.date : stageDay;
    strip?.querySelector(`[data-day="${day}"]`)?.scrollIntoView({ block: "nearest", inline: "nearest" });
  });

  async function confirmRemove() {
    const day = stageDay;
    await doc.change(tx => removeFrame(tx, doc.current, day));
    removing = false;
    picked = null;
    notify(`Removed ${shortLabel(day)}. Its photo leaves the file when it closes.`);
  }

  function keydown(event: KeyboardEvent) {
    if (event.key !== "Escape" || event.defaultPrevented || removing) return;
    if (mode === "camera") closeCamera();
    else if (mode === "play") stopPlay();
  }
  function visibilityChanged() {
    visible = document.visibilityState === "visible";
    if (visible) { now = new Date(); return; }
    // Never keep the camera on behind a hidden or minimized window.
    if (mode === "camera" && camera.phase !== "error") {
      closeCamera();
      notify("The camera turned off while the window was hidden.");
    }
  }
  $effect(() => {
    if (!visible) return;
    const timer = setInterval(() => (now = new Date()), 60_000);
    return () => clearInterval(timer);
  });
  onMount(() => {
    strip?.scrollTo({ left: strip.scrollWidth });
    return () => stopCamera();
  });
</script>

<svelte:window onkeydown={keydown} onpagehide={stopCamera} />
<svelte:document onvisibilitychange={visibilityChanged} />

<main class="df">
  <header class="df-head">
    <div class="df-heading">
      <p class="df-eyebrow">Daily Frame</p>
      <EditableText field={doc.fields.title} label="What the frames follow" placeholder="What are you photographing?" class="df-title" />
    </div>
    <dl class="df-readout" aria-label="The stack so far">
      <div><dt>frames</dt><dd>{view.stack.length}</dd></div>
      <div><dt>day run</dt><dd>{view.streak}</dd></div>
    </dl>
  </header>

  <section class="df-stage" aria-label={mode === "camera" ? "Camera" : mode === "play" ? "Flipbook" : `Frame for ${dayLabel(stageDay)}`}>
    <figure class="df-print" data-mode={mode}>
      <div class="df-window">
        {#if mode === "camera"}
          {#if camera.phase === "error"}
            <div class="df-problem" role="alert">
              <CameraOff size={30} aria-hidden="true" />
              <h2>{camera.title}</h2>
              <p>{camera.detail}</p>
              <div class="df-problem-actions">
                <button class="df-button df-button-light" onclick={() => startCamera()}>Try again</button>
                <button class="df-button df-button-ghost" onclick={() => fileInput?.click()} disabled={importing}><ImageUp size={16} aria-hidden="true" />Use a photo file</button>
              </div>
            </div>
          {:else}
            <!-- svelte-ignore a11y_media_has_caption -->
            <video bind:this={video} class="df-video" playsinline muted aria-label="Live camera preview"></video>
            {#if ghost && ghostUrl && camera.phase === "live"}
              <img class="df-ghost" src={ghostUrl} alt="" aria-hidden="true" />
            {/if}
            {#if camera.phase === "starting"}
              <p class="df-hint" role="status">Asking for the camera…</p>
            {:else if camera.phase === "live"}
              <p class="df-live" role="status"><span class="df-live-dot" aria-hidden="true"></span>Camera on</p>
              <span class="df-imprint df-imprint-live" aria-hidden="true">{stamp(today)}</span>
            {:else if camera.phase === "saving"}
              <img class="df-still" src={camera.still} alt="" />
              <p class="df-hint" role="status">Keeping today's frame…</p>
            {/if}
            {#if flash}<span class="df-flash" aria-hidden="true"></span>{/if}
          {/if}
        {:else if mode === "play" && playing}
          {#each view.stack as shown, at (shown.key)}
            {#if Math.abs(at - index) <= 1 || (index === view.stack.length - 1 && at === 0)}
              <span class="df-layer" class:df-layer-on={at === index}><Photo {shown} url={attachments.url} /></span>
            {/if}
          {/each}
        {:else if stageFrame}
          <Photo shown={stageFrame} url={attachments.url} onerror={() => notify("This photo couldn't be opened.")} />
        {:else}
          <div class="df-blank">
            {#if view.latest}<span class="df-blank-ghost" aria-hidden="true"><Photo shown={view.latest} url={attachments.url} imprint={false} /></span>{/if}
            <span class="df-imprint df-imprint-blank" aria-hidden="true">{stamp(today)}</span>
            <p>Today's frame isn't taken yet.</p>
            <p class="df-blank-sub">{view.previous ? "Line it up with the last one: the camera shows it faintly." : "Same place, same angle, every day."}</p>
          </div>
        {/if}
      </div>
      <figcaption class="df-caption">
        {#if mode === "camera" && camera.phase === "error"}
          <span class="df-caption-note df-caption-quiet">Camera off · nothing was saved.</span>
          <span class="df-caption-day">Today</span>
        {:else if mode === "camera"}
          <span class="df-caption-note">Live preview · nothing is saved until you press the shutter.</span>
          <span class="df-caption-day">Today</span>
        {:else if mode === "play" && playing}
          <span class="df-caption-note">{playing.note || " "}</span>
          <span class="df-caption-day" aria-live="off">{index + 1} / {view.stack.length}</span>
        {:else if stageFrame?.frame}
          <EditableText field={doc.at(stageFrame.frame).note} label={`Note for ${dayLabel(stageDay)}`} placeholder="Add a note…" class="df-caption-note df-note" />
          <span class="df-caption-day">{stageDay === today ? "Today" : dayLabel(stageDay)}</span>
        {:else if stageFrame}
          <span class="df-caption-note">{stageFrame.note || "Sample frame"}</span>
          <span class="df-caption-day">{dayLabel(stageDay)}</span>
        {:else}
          <span class="df-caption-note df-caption-quiet">{dayLabel(today)}</span>
          <span class="df-caption-day">Today</span>
        {/if}
      </figcaption>
    </figure>
  </section>

  <div class="df-controls" data-mode={mode}>
    {#if mode === "camera"}
      <button class="df-button df-button-ghost" onclick={closeCamera}><X size={16} aria-hidden="true" />Cancel</button>
      <button class="df-shutter" onclick={shutter} disabled={camera.phase !== "live"} aria-label="Take the photo"><span aria-hidden="true"></span></button>
      <div class="df-camera-tools">
        {#if ghostUrl}
          <Toggle.Root class="df-chip" bind:pressed={ghost} aria-label="Show the last frame faintly"><Layers size={15} aria-hidden="true" />Ghost</Toggle.Root>
        {/if}
        {#if cameras.length > 1}
          <Select.Root type="single" value={cameraId} items={cameras} onValueChange={value => { if (value && value !== cameraId) void startCamera(value); }}>
            <Select.Trigger class="df-chip" aria-label="Camera">{cameras.find(item => item.value === cameraId)?.label ?? "Camera"}<ChevronDown size={14} aria-hidden="true" /></Select.Trigger>
            <Select.Portal>
              <Select.Content class="df-menu" sideOffset={6}>
                <Select.Viewport>
                  {#each cameras as item (item.value)}
                    <Select.Item class="df-menu-item" value={item.value} label={item.label}>{item.label}</Select.Item>
                  {/each}
                </Select.Viewport>
              </Select.Content>
            </Select.Portal>
          </Select.Root>
        {/if}
      </div>
    {:else if mode === "play"}
      <button class="df-button df-button-ghost" onclick={stopPlay}><Square size={14} aria-hidden="true" />Stop</button>
      <button class="df-icon-button" onclick={() => (running = !running)} aria-label={running ? "Pause" : "Play"}>
        {#if running}<Pause size={18} aria-hidden="true" />{:else}<Play size={18} aria-hidden="true" />{/if}
      </button>
      <Slider.Root type="single" class="df-scrub" min={0} max={view.stack.length - 1} step={1} value={index}
        onValueChange={value => { running = false; index = value; }} aria-label="Flipbook position">
        {#snippet children({ thumbs })}
          <span class="df-scrub-track"><Slider.Range class="df-scrub-range" /></span>
          {#each thumbs as thumb (thumb)}<Slider.Thumb index={thumb} class="df-scrub-thumb" aria-label={playing ? `Frame ${index + 1}, ${dayLabel(playing.date)}` : "Frame"} />{/each}
        {/snippet}
      </Slider.Root>
      <ToggleGroup.Root type="single" class="df-speed" value={fps} onValueChange={value => { if (value) fps = value; }} aria-label="Frames per second">
        {#each ["3", "6", "12"] as rate (rate)}<ToggleGroup.Item value={rate} class="df-speed-item" aria-label={`${rate} frames per second`}>{rate}</ToggleGroup.Item>{/each}
        <span class="df-speed-unit" aria-hidden="true">fps</span>
      </ToggleGroup.Root>
    {:else}
      <button class="df-take" onclick={() => startCamera()} disabled={importing}>
        <span class="df-take-dot" aria-hidden="true"></span>{hasTodayPhoto ? "Retake today" : "Take today's frame"}
      </button>
      <button class="df-button df-button-ghost" onclick={startPlay} disabled={view.stack.length < 2}><Play size={15} aria-hidden="true" />Flipbook</button>
      <span class="df-spacer"></span>
      <button class="df-icon-button" onclick={() => fileInput?.click()} disabled={importing} aria-label="Use a photo file for today" title="Use a photo file for today"><ImageUp size={17} aria-hidden="true" /></button>
      {#if stageFrame?.frame}
        <button class="df-icon-button" onclick={() => (removing = true)} aria-label={`Remove the frame from ${dayLabel(stageDay)}`} title="Remove this frame"><Trash size={17} aria-hidden="true" /></button>
      {/if}
    {/if}
  </div>

  <section class="df-lower">
    <button class="df-ago" onclick={() => earlier && pick(earlier.date)} disabled={!earlier} aria-label={earlier ? `One month earlier: ${dayLabel(earlier.date)}` : `No frame near ${dayLabel(earlierDay)}`}>
      <span class="df-ago-photo">{#if earlier}<Photo shown={earlier} url={attachments.url} imprint={false} lazy />{:else}<span class="df-ago-empty">—</span>{/if}</span>
      <span class="df-ago-label">{stageDay === today ? "This day last month" : "A month before"}</span>
      <span class="df-ago-date">{earlier ? shortLabel(earlier.date) : `None near ${shortLabel(earlierDay)}`}</span>
    </button>
    <div class="df-strip-wrap">
      <p class="df-strip-head">
        {#if view.sample}<span class="df-sample-tag">Sample</span> drawn frames · your first photo replaces them
        {:else}{view.frames.length} {view.frames.length === 1 ? "frame" : "frames"} since {shortLabel(view.first?.date ?? today)} · {sizeLabel(view.bytes)}{#if view.full} · full{/if}{/if}
      </p>
      <div class="df-strip" bind:this={strip} role="toolbar" tabindex="-1" aria-label="The stack, oldest first" onkeydown={stripKeys}>
        {#each view.stack as shown (shown.key)}
          {@const current = mode === "play" && playing ? playing.date === shown.date : stageDay === shown.date}
          <button class="df-thumb" data-day={shown.date} aria-current={current ? "true" : undefined} tabindex={stageDay === shown.date ? 0 : -1}
            onclick={() => pick(shown.date)} aria-label={`${dayLabel(shown.date)}${shown.note ? `, ${shown.note}` : ""}`}>
            <span class="df-thumb-photo"><Photo {shown} url={attachments.url} imprint={false} lazy /></span>
            <span class="df-thumb-day">{newMonth(shown.date) ? shortLabel(shown.date) : shown.date.slice(8)}</span>
          </button>
        {/each}
        {#if !view.todayFrame}
          <button class="df-thumb df-thumb-today" data-day={today} aria-current={stageDay === today && mode !== "play" ? "true" : undefined} tabindex={stageDay === today ? 0 : -1}
            onclick={() => pick(today)} aria-label={`Today, ${dayLabel(today)}: no frame yet`}>
            <span class="df-thumb-photo"><span>+</span></span>
            <span class="df-thumb-day">{newMonth(today) ? shortLabel(today) : today.slice(8)}</span>
          </button>
        {/if}
      </div>
    </div>
  </section>

  <input bind:this={fileInput} class="df-file" type="file" accept="image/*" tabindex="-1" aria-hidden="true" onchange={fileChosen} />
</main>

<AlertDialog.Root open={removing} onOpenChange={open => (removing = open)}>
  <AlertDialog.Portal>
    <AlertDialog.Overlay class="df-overlay" />
    <AlertDialog.Content class="df-dialog">
      <AlertDialog.Title class="df-dialog-title">Remove the frame from {dayLabel(stageDay)}?</AlertDialog.Title>
      <AlertDialog.Description class="df-dialog-text">The photo and its note leave the stack. You can take a new one only for today.</AlertDialog.Description>
      <div class="df-dialog-actions">
        <AlertDialog.Cancel class="df-button df-button-light">Keep it</AlertDialog.Cancel>
        <AlertDialog.Action class="df-button df-button-danger" onclick={confirmRemove}>Remove frame</AlertDialog.Action>
      </div>
    </AlertDialog.Content>
  </AlertDialog.Portal>
</AlertDialog.Root>
