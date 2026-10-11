<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { onMount } from "svelte";
  import { attachments, capture, resizeWindow } from "hitslop/svelte";
  import { Toggle } from "bits-ui";
  import doc, { bands } from "./schema";
  import { Receiver } from "./player";
  import { resolveStations, type ResolvedStation } from "./stations";
  import { validateClassicSkin } from "./skin-archive";
  import {
    receiverView, saveAudio as commitAudio, selectStation, setSkin, setVisualizer as saveVisualizer, useBaseSkin,
    type AudioPreferences,
  } from "./settings";
  const capturing = capture.isRenderer();
  let stage: HTMLDivElement;
  let picker: HTMLInputElement;
  let receiver = $state.raw<Receiver | null>(null);
  let resolved: ResolvedStation[] = [];
  let status = $state("Starting receiver…");
  let error = $state<string | null>(null);
  let busy = $state(false);
  let visualizerOpen = $state(false);
  let changingSize = false;
  let webgl = $state(false);
  let reduced = $state(false);
  let dragActive = $state(false);
  // The skin Webamp shows: undefined is the base skin, null is unknown and must be reapplied.
  let appliedSkin: string | undefined | null = undefined;
  let failedSkin: string | undefined;
  let syncingSkin = false;
  let disposed = false;
  // Set when no SomaFM playlist answered at startup; stream failures then read as offline.
  let directoryDown = false;
  const view = $derived(receiverView(doc.current));

  const messageOf = (cause: unknown) => cause instanceof Error ? cause.message : String(cause);
  function message(text: string, failed = false) {
    if (disposed) return;
    status = text;
    error = failed ? text : null;
  }
  let audioTimer: ReturnType<typeof setTimeout> | undefined;
  // Webamp reports every slider step. Preview each one and commit once the drag settles;
  // pending previews also commit on flush, close and export.
  function saveAudio(value: AudioPreferences) {
    const { volume, balance, equalizer } = doc.fields;
    volume.preview(value.volume);
    balance.preview(value.balance);
    equalizer.on.preview(value.equalizer.on);
    for (const key of bands) equalizer[key].preview(value.equalizer[key]);
    clearTimeout(audioTimer);
    audioTimer = setTimeout(() => {
      if (disposed) return;
      void doc.change(tx => commitAudio(tx, value));
    }, 400);
  }
  async function setVisualizer(open: boolean, persist = true) {
    if (!receiver || changingSize || disposed) return;
    if (open && (!webgl || reduced)) {
      receiver.setVisualizer(false);
      message(reduced ? "MilkDrop is off while Reduce Motion is enabled." : "MilkDrop needs WebGL, which is unavailable in this view.", true);
      return;
    }
    changingSize = true;
    const previous = visualizerOpen;
    try {
      const size = { width: open ? 725 : 275, height: 470 };
      if (open && screen.availWidth < size.width) throw new Error("There isn’t enough screen space for MilkDrop.");
      // The native host widens the window; browser preview ignores the request.
      await resizeWindow(size);
      if (disposed) return;
      visualizerOpen = open;
      receiver.setVisualizer(open);
      if (persist && doc.current.milkdropOpen !== open) void doc.change(tx => saveVisualizer(tx, open));
    } catch (cause) {
      receiver?.setVisualizer(previous);
      message(messageOf(cause), true);
    } finally { changingSize = false; }
  }
  function baseSkin() {
    if (busy || syncingSkin) return;
    receiver?.baseSkin();
    appliedSkin = undefined;
    void doc.change(tx => useBaseSkin(tx, doc.current));
    message("Classic base skin");
  }
  async function savedSkin(id: string) {
    const blob = await attachments.read(id);
    await validateClassicSkin(new File([blob], "saved.wsz"));
    return blob;
  }
  /** Converge Webamp on the saved skin; an id that changes mid-load is picked up by the next pass. */
  async function syncSkin() {
    if (!receiver || syncingSkin || busy) return;
    syncingSkin = true;
    try {
      for (let id = doc.current.skin.id; !disposed && !busy && id !== appliedSkin && id !== failedSkin; id = doc.current.skin.id) {
        try {
          if (id) await receiver.loadSkin(await savedSkin(id));
          else receiver.baseSkin();
          appliedSkin = id;
          failedSkin = undefined;
        } catch (cause) {
          if (disposed) return;
          receiver.baseSkin();
          appliedSkin = undefined;
          failedSkin = id;
          message(`Could not restore skin: ${messageOf(cause)}. Choose a skin or use Base.`, true);
        }
      }
    } finally { syncingSkin = false; }
  }
  async function importSkin(file: File) {
    if (!receiver || busy || syncingSkin) return;
    busy = true;
    error = null;
    status = "Checking skin…";
    let skinChanged = false;
    try {
      await validateClassicSkin(file);
      if (disposed) return;
      status = "Applying skin…";
      skinChanged = true;
      await receiver.loadSkin(file);
      if (disposed) return;
      status = "Importing skin…";
      let importedId = "";
      await attachments.import<typeof doc.descriptor>(file, (tx, ref) => {
        setSkin(tx, ref);
        importedId = ref.id;
      });
      appliedSkin = importedId;
      failedSkin = undefined;
      message("Skin applied");
    } catch (cause) {
      // syncSkin restores the saved skin.
      if (skinChanged) appliedSkin = null;
      message(messageOf(cause), true);
    } finally {
      busy = false;
      void syncSkin();
    }
  }
  function inputChanged(event: Event) {
    const input = event.target;
    if (!(input instanceof HTMLInputElement) || input.type !== "file") return;
    const file = input.files?.[0];
    event.stopImmediatePropagation();
    if (file) void importSkin(file);
    input.value = "";
  }
  function drop(event: DragEvent) {
    event.preventDefault(); event.stopImmediatePropagation(); dragActive = false;
    const file = event.dataTransfer?.files[0];
    if (file) void importSkin(file);
  }
  function drag(event: DragEvent) {
    event.preventDefault(); event.stopImmediatePropagation();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "copy";
    dragActive = true;
  }
  function lockPanels(event: Event) {
    const target = event.target;
    if (target instanceof HTMLElement && (target.classList.contains("draggable") || target.id.endsWith("resize-target")))
      event.stopPropagation();
  }
  $effect(() => {
    if (receiver) {
      receiver.applyAudio({ volume: doc.current.volume, balance: doc.current.balance, equalizer: doc.current.equalizer });
      receiver.selectStation(doc.current.selectedStationId, resolved);
    }
  });
  $effect(() => { if (receiver && doc.current.skin.id !== appliedSkin) void syncSkin(); });
  $effect(() => {
    if (receiver) void setVisualizer(doc.current.milkdropOpen && !reduced, false);
  });
  onMount(() => {
    if (capturing) return;
    const abort = new AbortController();
    const motion = matchMedia("(prefers-reduced-motion: reduce)");
    const motionChanged = () => { reduced = motion.matches; receiver?.reducedMotion(reduced); };
    motionChanged();
    motion.addEventListener("change", motionChanged);
    const canvas = document.createElement("canvas");
    const context = canvas.getContext("webgl2");
    webgl = Boolean(context);
    context?.getExtension("WEBGL_lose_context")?.loseContext();
    document.addEventListener("change", inputChanged, true);
    stage.addEventListener("drop", drop, true);
    stage.addEventListener("dragover", drag, true);
    stage.addEventListener("mousedown", lockPanels, true);
    stage.addEventListener("touchstart", lockPanels, true);
    stage.addEventListener("dblclick", lockPanels, true);
    const leave = () => { dragActive = false; };
    stage.addEventListener("dragleave", leave);
    // Streams come from SomaFM over HTTPS; say so plainly when the network is gone.
    const offline = () => message("Offline · SomaFM streams need a network connection.", true);
    const online = () => message("Back online · press play");
    addEventListener("offline", offline);
    addEventListener("online", online);
    void (async () => {
      try {
        const saved = doc.current.skin.id;
        // Resolve streams and check the saved skin together so the receiver paints once, already skinned.
        const [streams, skin] = await Promise.all([
          resolveStations(abort.signal),
          saved ? savedSkin(saved).catch(cause => {
            failedSkin = saved;
            message(`Could not restore skin: ${messageOf(cause)}. Choose a skin or use Base.`, true);
            return undefined;
          }) : undefined,
        ]);
        abort.signal.throwIfAborted();
        resolved = streams;
        directoryDown = streams.every(station => station.usedFallback);
        const created = await Receiver.create(stage, resolved, doc.current, skin, {
          audio: saveAudio,
          station: id => { if (id !== doc.current.selectedStationId) void doc.change(tx => selectStation(tx, id)); },
          visualizer: open => { void setVisualizer(open); },
          baseSkin,
          status: (text, failed) => message(failed && (directoryDown || !navigator.onLine)
            ? "SomaFM is unreachable. Check your connection, then press play to retry." : text, failed),
        }, abort.signal);
        if (skin && created.skinApplied) appliedSkin = saved;
        else if (skin) {
          failedSkin = saved;
          message("Could not restore skin: Webamp could not apply it. Choose a skin or use Base.", true);
        }
        receiver = created.receiver;
        receiver.reducedMotion(reduced);
        if (!navigator.onLine) offline();
        else if (!error) message(directoryDown ? "SomaFM directory unreachable · press play to try saved streams" : "Ready · press play");
      } catch (cause) { if (!disposed) message(messageOf(cause), true); }
    })();
    return () => {
      disposed = true; clearTimeout(audioTimer); abort.abort(); receiver?.dispose();
      motion.removeEventListener("change", motionChanged);
      document.removeEventListener("change", inputChanged, true);
      stage.removeEventListener("drop", drop, true);
      stage.removeEventListener("dragover", drag, true);
      stage.removeEventListener("mousedown", lockPanels, true);
      stage.removeEventListener("touchstart", lockPanels, true);
      stage.removeEventListener("dblclick", lockPanels, true);
      stage.removeEventListener("dragleave", leave);
      removeEventListener("offline", offline);
      removeEventListener("online", online);
    };
  });
</script>

  <main class="soma-amp" class:soma-wide={visualizerOpen} aria-label="SomaAmp radio receiver">
    <div class="soma-stage" bind:this={stage}>
      {#if !receiver}<div class="soma-start"><strong>SOMA<span>AMP</span></strong><span>{status}</span></div>{/if}
    </div>
    {#if dragActive}<div class="soma-drop">Drop a classic .wsz skin</div>{/if}
    {#if error}<div class="soma-error" role="alert">{error}<button aria-label="Dismiss message" onclick={() => error = null}>×</button></div>{/if}
    <footer class="soma-rail">
      <div class="soma-rail-row">
      <a href="https://somafm.com/support/" target="_blank" rel="noreferrer" title="Support listener-funded SomaFM">SomaFM ↗</a>
      <span class="soma-status" role="status" title={status}>{status}</span>
      <Toggle.Root class="soma-action" pressed={visualizerOpen} onPressedChange={(open) => void setVisualizer(open)} disabled={!receiver || !webgl || reduced} aria-label="MilkDrop visualizer" title={reduced ? "Reduce Motion is enabled" : "Toggle MilkDrop visualizer"}>VIS</Toggle.Root>
      </div>
      <div class="soma-rail-row soma-skin-controls">
      <button class="soma-action" disabled={!receiver || busy} onclick={() => picker.click()} title="Import a classic Winamp skin">Import skin…</button>
      <a class="soma-action" href="https://skins.webamp.org/" target="_blank" rel="noreferrer" title="Browse the Winamp Skin Museum in your browser">Find skins ↗</a>
      <button class="soma-action" disabled={!receiver || busy || !view.hasSkin} onclick={baseSkin} title="Restore the classic base skin">Base</button>
      </div>
      <input bind:this={picker} type="file" accept=".wsz,.zip,application/zip" hidden aria-label="Choose a classic Winamp skin" />
    </footer>
  </main>
