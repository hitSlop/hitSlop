<script lang="ts">
  // The live mirror. Three layers per frame: the camera, mirrored; a hidden canvas that
  // stores only fog density; and frost (the frame blurred, brightened and tinted) masked by
  // that density with destination-in. Wiping erases density with destination-out; nothing
  // is ever painted white. All of it is view state and is gone when the mirror closes.
  import { onMount } from "svelte";
  import { Popover, Slider } from "bits-ui";
  import { attachments, notify } from "hitslop/svelte";
  import doc, { BRUSH } from "./schema";
  import { keepSnapshot, MAX_SNAPSHOT_BYTES, setBrush, snapshotName } from "./mirror";
  import { createTracker, type Delegate, type Tracker } from "./mediapipe";
  import {
    TUNING, breathBlobs, distance, ease, handWidth, isFist, keyQuiet, midpoint, mouthOf, mouthTarget, palmCentre, penIdle,
    pinchPoint, pinchRatio, pixelRatio, rms, stepBreath, stepPen, wipeStamps, type PenState, type Point,
  } from "./gestures";

  let { video: stream, audio, context, micProblem, onexit }: {
    video: MediaStream;
    audio: MediaStream | null;
    context: AudioContext | null;
    micProblem: string;
    onexit: () => void;
  } = $props();

  let canvas = $state<HTMLCanvasElement>();
  let videoEl = $state<HTMLVideoElement>();
  let tracking = $state<"waking" | "on" | "failed">("waking");
  let trackingError = $state("");
  let delegate = $state<Delegate | "">("");
  let handSeen = $state(false);
  let drawing = $state(false);
  let fogging = $state(false);
  let flash = $state(false);
  let saving = $state(false);
  let helpOpen = $state(false);
  const brushLabel = $props.id();
  let liveBrush = $state<number | null>(null);
  const brush = $derived(liveBrush ?? doc.current.brush);

  // Canvases and per-frame state. Sizes are device pixels: CSS pixels × min(dpr, 2).
  const fog = document.createElement("canvas");
  const frost = document.createElement("canvas");
  const small = document.createElement("canvas");
  const tiny = document.createElement("canvas");
  // Condensation and surface reflections are baked on resize, never regenerated per frame.
  const moisture = document.createElement("canvas");
  const glass = document.createElement("canvas");
  const fogCtx = fog.getContext("2d")!;
  const frostCtx = frost.getContext("2d")!;
  const smallCtx = small.getContext("2d")!;
  const tinyCtx = tiny.getContext("2d")!;
  let ctx: CanvasRenderingContext2D;
  let W = 0, H = 0, dpr = 1;
  let frostTint = "#f0f6f2";
  // Canvas filters (blur, brightness) where the engine has them; otherwise a downscale
  // blur and an additive pass for the 1.18 brightness.
  const filters = (() => { smallCtx.filter = "blur(2px)"; const ok = smallCtx.filter === "blur(2px)"; smallCtx.filter = "none"; return ok; })();

  // The trackers live only while the devices are on: created on mount, closed on unmount.
  let tracker: Tracker | null = null;
  let frame = 0;
  let raf = 0;
  let tracked = 0;
  let lastVideoTime = -1;
  let lastMouth: Point | null = null;
  let lastFaceAt = -Infinity;
  let floor: number | undefined;
  let lastKeyAt = -Infinity;
  let spaceHeld = false;
  let centre: Point | null = null;
  let analyser: AnalyserNode | null = null;
  let samples: Float32Array<ArrayBuffer> | null = null;

  // Hand state, in device pixels on the mirrored screen.
  let hand: Point[] | undefined;
  let pen: PenState = penIdle;
  let raw: Point | null = null;
  let target: Point | null = null;
  let tip: Point | null = null;
  let lastDrawn: Point | null = null;
  let lastMid: Point | null = null;
  let star: Point | null = null;
  let lastStamp: Point | undefined;
  let waitingForRelease = false;
  let neutralFrames = 0;

  function resetGesture() {
    pen = { ...penIdle };
    raw = target = tip = lastDrawn = lastMid = star = null;
    lastStamp = undefined;
    drawing = false;
    handSeen = false;
  }

  function cover() {
    const v = videoEl!;
    const vw = v.videoWidth || 1280, vh = v.videoHeight || 720;
    const scale = Math.max(W / vw, H / vh);
    const dw = vw * scale, dh = vh * scale;
    return { ox: (W - dw) / 2, oy: (H - dh) / 2, dw, dh };
  }
  // Landmarks and keypoints are normalized to the unmirrored camera frame.
  function toScreen(x: number, y: number): Point {
    const { ox, oy, dw, dh } = cover();
    return { x: W - (ox + x * dw), y: oy + y * dh };
  }
  function drawMirrored(target2d: CanvasRenderingContext2D, w: number, h: number) {
    const { ox, oy, dw, dh } = cover();
    const k = w / W;
    target2d.save();
    target2d.translate(w, 0);
    target2d.scale(-1, 1);
    target2d.drawImage(videoEl!, ox * k, oy * (h / H), dw * k, dh * (h / H));
    target2d.restore();
  }

  function resetFog() {
    fogCtx.globalCompositeOperation = "source-over";
    fogCtx.clearRect(0, 0, fog.width, fog.height);
    fogCtx.fillStyle = `rgba(0, 0, 0, ${TUNING.fogOnLoad})`;
    fogCtx.fillRect(0, 0, fog.width, fog.height);
  }

  function buildGlass() {
    moisture.width = glass.width = W;
    moisture.height = glass.height = H;
    const wet = moisture.getContext("2d")!;
    const pane = glass.getContext("2d")!;
    // A fixed seed keeps the same droplets when the pane is resized.
    let seed = 7183;
    const random = () => { seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0; return seed / 4294967296; };
    const short = Math.min(W, H);
    for (let i = 0; i < 24; i++) {
      const x = random() * W, y = random() * H;
      const r = short * (0.08 + random() * 0.25);
      const haze = wet.createRadialGradient(x, y, 0, x, y, r);
      haze.addColorStop(0, `rgba(255,255,255,${0.018 + random() * 0.035})`);
      haze.addColorStop(1, "rgba(255,255,255,0)");
      wet.fillStyle = haze;
      wet.fillRect(x - r, y - r, r * 2, r * 2);
    }
    // Pinpoint highlights with a shaded lower edge read as water, not animated noise.
    const count = Math.min(6500, Math.round((W / dpr) * (H / dpr) / 95));
    for (let i = 0; i < count; i++) {
      const x = random() * W, y = random() * H;
      const edge = Math.abs(x / W - 0.5) * 2;
      const r = (0.25 + Math.pow(random(), 5) * (0.8 + edge)) * dpr;
      wet.fillStyle = `rgba(30,65,55,${0.025 + random() * 0.035})`;
      wet.beginPath(); wet.ellipse(x, y + r * 0.4, r, r * 1.15, 0, 0, Math.PI * 2); wet.fill();
      wet.fillStyle = `rgba(255,255,255,${0.10 + random() * 0.13})`;
      wet.beginPath(); wet.ellipse(x - r * 0.2, y - r * 0.3, r * 0.7, r * 0.5, -0.5, 0, Math.PI * 2); wet.fill();
    }

    // An almost invisible reflection and polished lip locate the surface of the glass.
    const sheen = pane.createLinearGradient(0, 0, W, H * 0.7);
    sheen.addColorStop(0, "rgba(255,255,255,0.10)");
    sheen.addColorStop(0.24, "rgba(255,255,255,0.015)");
    sheen.addColorStop(0.48, "rgba(255,255,255,0)");
    sheen.addColorStop(1, "rgba(255,255,255,0)");
    pane.fillStyle = sheen;
    pane.fillRect(0, 0, W, H);
    const edgeLight = pane.createLinearGradient(0, 0, W, H);
    edgeLight.addColorStop(0, "rgba(255,255,255,0.65)");
    edgeLight.addColorStop(0.45, "rgba(255,255,255,0.08)");
    edgeLight.addColorStop(0.75, "rgba(25,55,45,0.12)");
    edgeLight.addColorStop(1, "rgba(255,255,255,0.35)");
    pane.strokeStyle = edgeLight;
    pane.lineWidth = dpr;
    pane.beginPath(); pane.roundRect(dpr, dpr, W - 2 * dpr, H - 2 * dpr, 21 * dpr); pane.stroke();
  }

  function resize() {
    if (!canvas) return;
    dpr = pixelRatio(window.devicePixelRatio);
    const w = Math.max(1, Math.round(canvas.clientWidth * dpr)), h = Math.max(1, Math.round(canvas.clientHeight * dpr));
    if (w === W && h === H) return;
    // Keep the fog the person made: scale the old density into the new size.
    const old = W ? (() => { const c = document.createElement("canvas"); c.width = fog.width; c.height = fog.height; c.getContext("2d")!.drawImage(fog, 0, 0); return c; })() : null;
    W = w; H = h;
    canvas.width = fog.width = frost.width = W;
    canvas.height = fog.height = frost.height = H;
    small.width = Math.max(1, Math.ceil(W / 4)); small.height = Math.max(1, Math.ceil(H / 4));
    tiny.width = Math.max(1, Math.ceil(W / 24)); tiny.height = Math.max(1, Math.ceil(H / 24));
    buildGlass();
    if (old) fogCtx.drawImage(old, 0, 0, W, H); else resetFog();
    centre = null;
  }

  // ---- Fog -----------------------------------------------------------------------------
  // The breath centres slightly below the mouth and glides there at 0.4 per frame.
  function followMouth(now: number) {
    const mouth = mouthTarget(lastMouth, lastFaceAt, now);
    const at = toScreen(mouth.x, mouth.y);
    const aim = { x: at.x, y: at.y + TUNING.breathBelowMouth * Math.min(W, H) };
    centre = centre ? ease(centre, aim, TUNING.centreEase) : aim;
  }

  function breathe(strength: number) {
    if (!centre) return;
    fogCtx.globalCompositeOperation = "source-over";
    for (const blob of breathBlobs(centre, Math.min(W, H), strength, Math.random)) {
      const g = fogCtx.createRadialGradient(blob.x, blob.y, 0, blob.x, blob.y, blob.r);
      g.addColorStop(0, `rgba(0, 0, 0, ${blob.alpha})`);
      g.addColorStop(1 - TUNING.breathBlur, `rgba(0, 0, 0, ${blob.alpha * 0.55})`);
      g.addColorStop(1, "rgba(0, 0, 0, 0)");
      fogCtx.fillStyle = g;
      fogCtx.beginPath();
      fogCtx.arc(blob.x, blob.y, blob.r, 0, Math.PI * 2);
      fogCtx.fill();
    }
  }

  function strokeTo(next: Point) {
    // Quadratic curves through midpoints: the last point is the control.
    const r = brush * dpr;
    fogCtx.globalCompositeOperation = "destination-out";
    fogCtx.strokeStyle = "rgba(0, 0, 0, 1)";
    fogCtx.fillStyle = "rgba(0, 0, 0, 1)";
    fogCtx.lineWidth = r * 2;
    fogCtx.lineCap = "round";
    fogCtx.lineJoin = "round";
    fogCtx.shadowColor = "rgba(0, 0, 0, 1)";
    fogCtx.shadowBlur = r * 0.6;
    const mid = midpoint(lastDrawn!, next);
    fogCtx.beginPath();
    fogCtx.moveTo(lastMid!.x, lastMid!.y);
    fogCtx.quadraticCurveTo(lastDrawn!.x, lastDrawn!.y, mid.x, mid.y);
    fogCtx.stroke();
    fogCtx.shadowBlur = 0;
    lastMid = mid;
    lastDrawn = next;
  }
  function dot(at: Point) {
    const r = brush * dpr;
    fogCtx.globalCompositeOperation = "destination-out";
    fogCtx.fillStyle = "rgba(0, 0, 0, 1)";
    fogCtx.beginPath();
    fogCtx.arc(at.x, at.y, r, 0, Math.PI * 2);
    fogCtx.fill();
  }
  function stampWipe(at: Point, radius: number) {
    const g = fogCtx.createRadialGradient(at.x, at.y, 0, at.x, at.y, radius);
    g.addColorStop(0, `rgba(0, 0, 0, ${TUNING.wipeAlpha})`);
    g.addColorStop(1, "rgba(0, 0, 0, 0)");
    fogCtx.globalCompositeOperation = "destination-out";
    fogCtx.fillStyle = g;
    fogCtx.beginPath();
    fogCtx.arc(at.x, at.y, radius, 0, Math.PI * 2);
    fogCtx.fill();
  }

  // ---- Tracking (synchronous, once per new camera frame) ---------------------------------
  function track(v: HTMLVideoElement, now: number) {
    if (!tracker || v.currentTime === lastVideoTime) return;
    lastVideoTime = v.currentTime;
    try {
      // The face is looked for on every 5th tracked frame; the hand on every one.
      if (tracked++ % TUNING.faceEvery === 0) {
        const mouth = mouthOf(tracker.faces(v, now), v.videoWidth, v.videoHeight);
        if (mouth) { lastMouth = mouth; lastFaceAt = now; }
      }
      onHand(tracker.hand(v, now));
    } catch (error) {
      tracking = "failed";
      trackingError = error instanceof Error ? error.message : String(error);
      resetGesture();
      console.error("Tracking stopped", error);
      tracker.close();
      tracker = null;
    }
  }

  function onHand(landmarks: readonly Point[] | undefined) {
    hand = landmarks ? landmarks.map(p => toScreen(p.x, p.y)) : undefined;
    handSeen = !!hand;
    // A held pinch/fist must relax before it can mark newly re-fogged glass.
    if (waitingForRelease) {
      neutralFrames = !hand || (pinchRatio(hand) > TUNING.pinchUp && !isFist(hand)) ? neutralFrames + 1 : 0;
      if (neutralFrames >= TUNING.releaseFrames) waitingForRelease = false;
      return;
    }
    const was = pen.down;
    pen = stepPen(pen, hand, performance.now());
    if (hand) raw = pinchPoint(hand);
    if (pen.down && !was && raw) {
      target = raw; tip = raw; lastDrawn = raw; lastMid = raw;
      dot(raw);
    }
    if (!pen.down) { target = tip = lastDrawn = lastMid = null; }
    drawing = pen.down;
    // A fist wipes when the pen is up.
    if (hand && !pen.down && isFist(hand)) {
      const at = palmCentre(hand);
      const radius = TUNING.wipeScale * handWidth(hand);
      const stamps = wipeStamps(lastStamp, at, radius, Math.max(W, H));
      for (const s of stamps) stampWipe(s, radius);
      if (stamps.length) lastStamp = stamps[stamps.length - 1];
    } else {
      lastStamp = undefined;
    }
  }

  // ---- The frame -------------------------------------------------------------------------
  function buildFrost() {
    smallCtx.globalCompositeOperation = "source-over";
    smallCtx.clearRect(0, 0, small.width, small.height);
    if (filters) {
      smallCtx.filter = `blur(${(18 * dpr * small.width) / W}px) brightness(1.18)`;
      drawMirrored(smallCtx, small.width, small.height);
      smallCtx.filter = "none";
    } else {
      tinyCtx.clearRect(0, 0, tiny.width, tiny.height);
      drawMirrored(tinyCtx, tiny.width, tiny.height);
      smallCtx.imageSmoothingQuality = "high";
      smallCtx.drawImage(tiny, 0, 0, small.width, small.height);
      smallCtx.globalCompositeOperation = "lighter";
      smallCtx.globalAlpha = 0.18;
      smallCtx.drawImage(tiny, 0, 0, small.width, small.height);
      smallCtx.globalAlpha = 1;
      smallCtx.globalCompositeOperation = "source-over";
    }
    frostCtx.globalCompositeOperation = "source-over";
    frostCtx.clearRect(0, 0, W, H);
    frostCtx.imageSmoothingQuality = "high";
    frostCtx.drawImage(small, 0, 0, W, H);
    frostCtx.fillStyle = frostTint;
    // Less white pigment; the stronger diffusion below carries the fog instead.
    frostCtx.globalAlpha = 0.42;
    frostCtx.fillRect(0, 0, W, H);
    frostCtx.globalAlpha = 1;
    frostCtx.drawImage(moisture, 0, 0);
    frostCtx.globalCompositeOperation = "destination-in";
    frostCtx.drawImage(fog, 0, 0);
    frostCtx.globalCompositeOperation = "source-over";
  }

  function render(target2d: CanvasRenderingContext2D, withStar: boolean) {
    target2d.clearRect(0, 0, W, H);
    drawMirrored(target2d, W, H);
    // Two optical passes: density 0.55 diffuses ~80% of the image, while a wiped
    // alpha of zero stays completely clear. Both use the same erasable fog mask.
    target2d.drawImage(frost, 0, 0);
    target2d.drawImage(frost, 0, 0);
    target2d.drawImage(glass, 0, 0);
    if (withStar && star && handSeen) {
      const s = 6 * dpr;
      target2d.save();
      target2d.translate(star.x, star.y);
      target2d.shadowColor = "rgba(255, 255, 255, 0.95)";
      target2d.shadowBlur = (drawing ? 16 : 9) * dpr;
      target2d.globalAlpha = drawing ? 1 : 0.65;
      target2d.fillStyle = "#ffffff";
      target2d.beginPath();
      for (let i = 0; i < 8; i++) {
        const r = i % 2 ? s * 0.32 : s;
        const a = (i * Math.PI) / 4 - Math.PI / 2;
        target2d.lineTo(Math.cos(a) * r, Math.sin(a) * r);
      }
      target2d.closePath();
      target2d.fill();
      target2d.restore();
    }
  }

  function tick(now: number) {
    raf = requestAnimationFrame(tick);
    const v = videoEl;
    if (!v || v.readyState < 2) return;
    resize();
    frame++;
    track(v, now);
    followMouth(now);

    // Breath: level against a floating noise floor, quiet for 300 ms after a key.
    let strength = 0;
    if (analyser && samples && keyQuiet(now, lastKeyAt)) {
      analyser.getFloatTimeDomainData(samples);
      const step = stepBreath(floor, rms(samples));
      floor = step.floor;
      strength = step.strength;
    }
    if (spaceHeld) strength = 1;
    fogging = strength > 0 && !drawing;
    if (fogging && frame % 2 === 0) breathe(strength);

    // Drawing: raw → target at 0.45, the brush chases the target at 0.22.
    if (pen.down && raw && target && tip && lastDrawn) {
      target = ease(target, raw, TUNING.rawToTarget);
      tip = ease(tip, target, TUNING.brushChase);
      if (distance(tip, lastDrawn) >= TUNING.minMove * W) strokeTo(tip);
    }
    if (raw) star = star ? ease(star, raw, TUNING.starChase) : raw;

    buildFrost();
    render(ctx, true);
  }

  // ---- Controls --------------------------------------------------------------------------
  async function save() {
    if (saving || !W) return;
    saving = true;
    flash = true;
    setTimeout(() => (flash = false), 180);
    try {
      const shot = document.createElement("canvas");
      shot.width = W; shot.height = H;
      render(shot.getContext("2d")!, false);
      let blob = await new Promise<Blob | null>(resolve => shot.toBlob(resolve, "image/png"));
      let ext = "png";
      if (blob && blob.size > MAX_SNAPSHOT_BYTES) {
        blob = await new Promise<Blob | null>(resolve => shot.toBlob(resolve, "image/jpeg", 0.9));
        ext = "jpg";
      }
      if (!blob) throw new Error("This window couldn't make an image of the mirror.");
      if (blob.size > MAX_SNAPSHOT_BYTES) throw new Error("This mirror is too large to save (over 10 MB). Make the window smaller and try again.");
      const now = new Date();
      const file = new File([blob], snapshotName(now, ext), { type: blob.type });
      await attachments.import<typeof doc.descriptor>(file, (tx, ref) => keepSnapshot(tx, ref, W, H, now.toISOString()));
      notify(`Saved · ${(blob.size / 1024 / 1024).toFixed(1)} MB ${ext.toUpperCase()}`);
    } catch (error) {
      notify(error instanceof Error ? error.message : "The snapshot was not saved.");
      throw error;
    } finally {
      saving = false;
    }
  }

  function clear() {
    resetGesture();
    waitingForRelease = true;
    neutralFrames = 0;
    resetFog();
    notify("Fogged over again.");
  }

  function keydown(event: KeyboardEvent) {
    lastKeyAt = performance.now();
    if (event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey) return;
    if (event.key === "Escape") {
      event.preventDefault();
      if (helpOpen) helpOpen = false;
      else onexit();
      return;
    }
    const target = event.target instanceof Element ? event.target : null;
    if (helpOpen || target?.closest('button, input, textarea, select, a, [role="slider"], [contenteditable="true"]')) return;
    if (event.code === "Space") {
      event.preventDefault();
      spaceHeld = true;
    } else if (!event.repeat && (event.key === "c" || event.key === "C")) {
      clear();
    } else if (!event.repeat && (event.key === "s" || event.key === "S")) {
      event.preventDefault();
      void save();
    }
  }
  function keyup(event: KeyboardEvent) {
    lastKeyAt = performance.now();
    if (event.code === "Space") {
      if (spaceHeld) event.preventDefault();
      spaceHeld = false;
    }
  }

  function commitBrush(value: number) {
    void doc.change(tx => setBrush(tx, value));
    liveBrush = null;
  }

  onMount(() => {
    ctx = canvas!.getContext("2d")!;
    const updateTint = () => { frostTint = getComputedStyle(canvas!).getPropertyValue("--slop-frost").trim() || "#f0f6f2"; };
    updateTint();
    const themeObserver = new MutationObserver(updateTint);
    themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ["style"] });
    videoEl!.srcObject = stream;
    void videoEl!.play();
    if (audio && context) {
      analyser = context.createAnalyser();
      analyser.fftSize = 1024;
      context.createMediaStreamSource(audio).connect(analyser);
      samples = new Float32Array(analyser.fftSize);
    }
    let alive = true;
    createTracker().then(t => {
      if (!alive) return t.close();
      tracker = t;
      delegate = t.delegate;
      tracking = "on";
    }, (error: unknown) => {
      if (!alive) return;
      tracking = "failed";
      trackingError = error instanceof Error ? error.message : String(error);
      console.error("Tracking did not start", error);
    });
    raf = requestAnimationFrame(tick);
    const blur = () => { spaceHeld = false; };
    window.addEventListener("blur", blur);
    return () => {
      alive = false;
      cancelAnimationFrame(raf);
      themeObserver.disconnect();
      tracker?.close();
      tracker = null;
      window.removeEventListener("blur", blur);
      analyser?.disconnect();
      if (videoEl) videoEl.srcObject = null;
    };
  });
</script>

<svelte:window onkeydown={keydown} onkeyup={keyup} />

<main class="fg-live" data-drawing={drawing || undefined} data-delegate={delegate || undefined}>
  <!-- svelte-ignore a11y_media_has_caption -->
  <video bind:this={videoEl} class="fg-video" muted playsinline aria-hidden="true"></video>
  <canvas bind:this={canvas} class="fg-canvas" aria-label="Your fogged mirror"></canvas>
  <div class="fg-flash" data-on={flash || undefined} aria-hidden="true"></div>

  <div class="fg-top">
    <button class="fg-pill fg-back" onclick={onexit} aria-label="Done — turn off the camera (Escape)">Done</button>
    <p class="fg-pill fg-live-pill" role="status">
      <span class="fg-dot" aria-hidden="true"></span><span>camera{audio ? " + mic" : ""} on</span>
    </p>
  </div>

  <div class="fg-actions">
    <button class="fg-pill" onclick={clear} title="Restore the fog (C)">Re-fog</button>
    <Popover.Root bind:open={helpOpen}>
      <Popover.Trigger class="fg-pill" aria-label="Help with the mirror">Help</Popover.Trigger>
      <Popover.Portal>
        <Popover.Content class="fg-help" sideOffset={10} collisionPadding={14} align="end"
          onEscapeKeydown={(event) => { event.preventDefault(); helpOpen = false; }}>
          <div class="fg-help-heading"><h2>A little mirror magic</h2><Popover.Close aria-label="Close help">×</Popover.Close></div>
          <dl class="fg-gestures">
            <div><dt>Breathe</dt><dd>Blow gently toward your mic to add fog.</dd></div>
            <div><dt>Pinch</dt><dd>Touch thumb to index finger, then draw.</dd></div>
            <div><dt>Wipe</dt><dd>Make a fist and sweep across the mirror.</dd></div>
          </dl>
          <div class="fg-shortcuts">
            <span><kbd>Space</kbd> Hold to fog</span><span><kbd>C</kbd> Re-fog</span>
            <span><kbd>S</kbd> Keep a snapshot</span><span><kbd>Esc</kbd> Done</span>
          </div>
          <p>Snapshots stay in this document. Use hitSlop’s export to take one with you.</p>
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  </div>

  {#if tracking !== "on" || micProblem}
    <div class="fg-status fg-pill" role="status">
      {#if tracking === "waking"}<p>Getting the mirror ready…</p>{/if}
      {#if tracking === "failed"}<p>Hand tracking stopped. Choose Done, then start again.</p><p class="fg-status-detail">{trackingError}</p>{/if}
      {#if micProblem}<p>{micProblem}</p>{/if}
    </div>
  {/if}

  <div class="fg-pill fg-brush">
    <span id={brushLabel}>brush</span>
    <Slider.Root type="single" class="fg-slider" min={BRUSH.min} max={BRUSH.max} step={1} value={brush}
      onValueChange={(value) => liveBrush = value} onValueCommit={commitBrush} aria-labelledby={brushLabel}>
      {#snippet children({ thumbs })}
        <span class="fg-slider-track"><Slider.Range class="fg-slider-range" /></span>
        {#each thumbs as index (index)}<Slider.Thumb {index} class="fg-slider-thumb" aria-label="Brush radius" />{/each}
      {/snippet}
    </Slider.Root>
    <output>{brush}</output>
  </div>

  <button class="fg-shutter" onclick={() => void save()} disabled={saving} aria-label="Save this frame (S)">
    <span aria-hidden="true"></span>
  </button>
</main>
