<script lang="ts">
  import { onMount } from "svelte";
  import { Slop, useDocument } from "@hitslop/document/svelte";
  import { attachments } from "@hitslop/document/attachments";
  import { capture } from "@hitslop/document/capture";
  import { Toggle } from "bits-ui";
  import schema, { bands } from "./schema";
  import { Receiver, type AudioPreferences } from "./player";
  import { stations, resolveStations, type ResolvedStation } from "./stations";
  import { validateClassicSkin } from "./skin-archive";

  const doc = useDocument(schema);
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
  const selected = $derived(stations.find(s => s.id === doc.current.selectedStationId) ?? stations[0]!);
  const skinName = $derived(doc.current.skin.name ?? "Classic base skin");

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
      doc.change(tx => {
        tx.fields.volume.set(value.volume);
        tx.fields.balance.set(value.balance);
        tx.fields.equalizer.on.set(value.equalizer.on);
        for (const key of bands) tx.fields.equalizer[key].set(value.equalizer[key]);
      }, { message: "Adjust audio" });
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
      const host = globalThis.slop?.window;
      if (host) {
        const applied = await host.resize(size);
        if (applied.width < size.width || applied.height < size.height) {
          await host.resize({ width: previous ? 725 : 275, height: 470 });
          throw new Error("There isn’t enough screen space for MilkDrop.");
        }
      }
      if (disposed) return;
      visualizerOpen = open;
      receiver.setVisualizer(open);
      if (persist && doc.current.milkdropOpen !== open) doc.fields.milkdropOpen.set(open);
    } catch (cause) {
      receiver?.setVisualizer(previous);
      message(messageOf(cause), true);
    } finally { changingSize = false; }
  }
  function baseSkin() {
    if (busy || syncingSkin) return;
    receiver?.baseSkin();
    appliedSkin = undefined;
    doc.change(tx => { tx.fields.skin.id.clear(); tx.fields.skin.name.clear(); });
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
      await attachments.import(file, { commit(ref) {
        doc.change(tx => { tx.fields.skin.id.set(ref.id); tx.fields.skin.name.set(ref.name); });
        appliedSkin = ref.id;
        failedSkin = undefined;
      } });
      message("Skin applied");
    } catch (cause) {
      // Disk failures stay in the native retry queue; otherwise syncSkin restores the saved skin.
      if (skinChanged && doc.status !== "save-failed") appliedSkin = null;
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
        const created = await Receiver.create(stage, resolved, doc.current, skin, {
          audio: saveAudio,
          station: id => { if (id !== doc.current.selectedStationId) doc.fields.selectedStationId.set(id); },
          visualizer: open => { void setVisualizer(open); },
          baseSkin,
          status: message,
        }, abort.signal);
        if (skin && created.skinApplied) appliedSkin = saved;
        else if (skin) {
          failedSkin = saved;
          message("Could not restore skin: Webamp could not apply it. Choose a skin or use Base.", true);
        }
        receiver = created.receiver;
        receiver.reducedMotion(reduced);
        if (!error) message("Ready · press play");
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
    };
  });
</script>

<Slop>
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
      <button class="soma-action" disabled={!receiver || busy || !doc.current.skin.id} onclick={baseSkin} title="Restore the classic base skin">Base</button>
      </div>
      <input bind:this={picker} type="file" accept=".wsz,.zip,application/zip" hidden aria-label="Choose a classic Winamp skin" />
    </footer>
  </main>
  {#snippet exportView()}
    <article class="soma-export">
      <header><span>LISTENER-SUPPORTED RADIO / SOMAFM</span><strong>SOMA<span>AMP</span></strong></header>
      <div class="soma-export-display">
        <span>SELECTED STATION</span><h1>{selected.title}</h1><p>{selected.genre}</p>
        <div class="soma-spectrum" aria-hidden="true">{#each [22, 43, 66, 91, 58, 76, 100, 69, 40, 24] as height}<i style={`height: ${height}%`}></i>{/each}</div>
      </div>
      <dl><div><dt>Skin</dt><dd>{skinName}</dd></div><div><dt>Receiver</dt><dd>{Math.round(doc.current.volume)}% · EQ {doc.current.equalizer.on ? "on" : "off"}</dd></div></dl>
    </article>
  {/snippet}
  {#snippet icon()}
    <div class="soma-icon"><div><span>▶</span><i></i><i></i><i></i><i></i><i></i></div></div>
  {/snippet}
</Slop>
