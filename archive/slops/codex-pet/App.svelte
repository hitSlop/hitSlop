<script lang="ts">
  import { attachments } from "@hitslop/document/attachments";
  import { capture } from "@hitslop/document/capture";
  import { Slop, useDocument } from "@hitslop/document/svelte";
  import { onMount, tick } from "svelte";
  import { Button } from "bits-ui";
  import { drawPetIcon } from "./icon-draw";
  import { parsePetArchive, parsePetPackage, validateSpriteImage, type PetMetadata } from "./pet-package";
  import schema from "./schema";

  type PetAction = "idle" | "running-right" | "running-left" | "waving" | "jumping" | "failed" | "waiting" | "running" | "review";
  type ActionSpec = { row: number; durations: number[] };

  const CELL_WIDTH = 192, CELL_HEIGHT = 208;
  const STARTER_SHEET = "/assets/bubu-spritesheet.webp";
  const STARTER: PetMetadata = {
    id: "bubu--gbn666",
    displayName: "Bubu",
    description: "A soft, sleepy milk-tea-brown chibi bear desktop pet.",
    spriteVersionNumber: 1,
    spritesheetPath: "bubu-spritesheet.webp",
  };
  const ACTIONS: Record<PetAction, ActionSpec> = {
    idle: { row: 0, durations: [280, 110, 110, 140, 140, 320] },
    "running-right": { row: 1, durations: [120, 120, 120, 120, 120, 120, 120, 220] },
    "running-left": { row: 2, durations: [120, 120, 120, 120, 120, 120, 120, 220] },
    waving: { row: 3, durations: [140, 140, 140, 280] },
    jumping: { row: 4, durations: [140, 140, 140, 140, 280] },
    failed: { row: 5, durations: [140, 140, 140, 140, 140, 140, 140, 240] },
    waiting: { row: 6, durations: [150, 150, 150, 150, 150, 260] },
    running: { row: 7, durations: [120, 120, 120, 120, 120, 220] },
    review: { row: 8, durations: [150, 150, 150, 150, 150, 280] },
  };

  const doc = useDocument(schema);
  const capturing = capture.isRenderer();
  let canvas: HTMLCanvasElement;
  let iconCanvas = $state<HTMLCanvasElement>();
  let exportCanvas = $state<HTMLCanvasElement>();
  let fileInput: HTMLInputElement;
  let activePet = $state<PetMetadata>(STARTER);
  let activeImage = $state.raw<HTMLImageElement | null>(null);
  let action = $state<PetAction>("idle");
  let frame = $state(0);
  let error = $state<string | null>(null);
  let dragActive = $state(false);
  let importing = $state(false);
  let reducedMotion = $state(false);
  let imageObjectUrl: string | null = null;
  let frameTimer = 0, ambientTimer = 0, loadGeneration = 0;
  let loadedPackageKey: string | null = null;

  function spriteBlob(bytes: Uint8Array, type: string): Blob {
    const copy = new Uint8Array(bytes.byteLength);
    copy.set(bytes);
    return new Blob([copy], { type });
  }

  function draw(): void {
    if (!canvas || !activeImage) return;
    const context = canvas.getContext("2d");
    if (!context) return;
    context.clearRect(0, 0, CELL_WIDTH, CELL_HEIGHT);
    const spec = ACTIONS[action];
    context.drawImage(activeImage, frame * CELL_WIDTH, spec.row * CELL_HEIGHT, CELL_WIDTH, CELL_HEIGHT, 0, 0, CELL_WIDTH, CELL_HEIGHT);
  }

  function play(next: PetAction, cycles = 1): void {
    window.clearTimeout(frameTimer);
    action = next; frame = 0; draw();
    if (reducedMotion || capturing) return;
    const spec = ACTIONS[next];
    let remaining = cycles;
    const advance = () => {
      frameTimer = window.setTimeout(() => {
        if (frame + 1 < spec.durations.length) frame += 1;
        else {
          if (Number.isFinite(remaining)) remaining -= 1;
          if (remaining <= 0 && next !== "idle") { play("idle", Number.POSITIVE_INFINITY); return; }
          frame = 0;
        }
        draw(); advance();
      }, spec.durations[frame] ?? 140);
    };
    advance();
  }

  function ambientPool(hour: number): PetAction[] {
    if (hour < 6 || hour >= 22) return ["waiting", "idle", "idle"];
    if (hour < 10) return ["waving", "jumping", "waving", "waiting"];
    if (hour < 17) return ["running", "review", "running", "waiting"];
    return ["waving", "review", "jumping", "waiting"];
  }

  function scheduleAmbient(): void {
    window.clearTimeout(ambientTimer);
    if (reducedMotion || capturing || document.hidden) return;
    const hour = new Date().getHours();
    const overnight = hour < 6 || hour >= 22;
    const delay = overnight ? 30_000 + Math.random() * 60_000 : 12_000 + Math.random() * 18_000;
    ambientTimer = window.setTimeout(() => {
      const pool = ambientPool(hour);
      play(pool[Math.floor(Math.random() * pool.length)] ?? "idle", 1);
      scheduleAmbient();
    }, delay);
  }

  async function adoptImage(metadata: PetMetadata, source: string, objectUrl: string | null, generation: number, rememberName: boolean): Promise<void> {
    const image = new Image();
    image.decoding = "async"; image.src = source;
    try { await image.decode(); }
    catch { throw new Error("The pet spritesheet could not be decoded."); }
    if (generation !== loadGeneration) { if (objectUrl) URL.revokeObjectURL(objectUrl); return; }
    if (imageObjectUrl) URL.revokeObjectURL(imageObjectUrl);
    imageObjectUrl = objectUrl;
    activePet = metadata; activeImage = image; error = null;
    if (rememberName && doc.current.displayName !== metadata.displayName) doc.fields.displayName.replace(metadata.displayName);
    await tick();
    if (generation !== loadGeneration) return;
    play("idle", Number.POSITIVE_INFINITY);
    scheduleAmbient();
  }

  async function loadStarter(generation: number): Promise<void> {
    await adoptImage(STARTER, STARTER_SHEET, null, generation, !doc.current.package);
  }

  async function loadStored(id: string, mimeType: string, generation: number): Promise<void> {
    try {
      const blob = await attachments.read(id, { type: mimeType });
      const parsed = await parsePetArchive(new Uint8Array(await blob.arrayBuffer()));
      await validateSpriteImage(parsed);
      const objectUrl = URL.createObjectURL(spriteBlob(parsed.spriteBytes, parsed.spriteMime));
      await adoptImage(parsed, objectUrl, objectUrl, generation, true);
    } catch (cause) {
      if (generation !== loadGeneration) return;
      const message = `Stored pet unavailable: ${cause instanceof Error ? cause.message : String(cause)}`;
      error = message;
      await loadStarter(generation);
      error = message;
    }
  }

  async function importPackage(file: File | undefined): Promise<void> {
    if (!file || importing) return;
    importing = true; error = null;
    try {
      const parsed = await parsePetPackage(file);
      await attachments.import(file, { commit(ref) {
        doc.change((tx) => {
          tx.fields.package.set({ id: ref.id, name: ref.name, mimeType: ref.mimeType, byteLength: ref.byteLength });
          tx.fields.displayName.replace(parsed.displayName);
        });
      } });
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally { importing = false; }
  }

  function useStarter(): void {
    if (!doc.current.package) return;
    doc.change((tx) => {
      tx.fields.package.clear();
      tx.fields.displayName.replace(STARTER.displayName);
    });
    error = null;
  }

  function paintPortrait(target: HTMLCanvasElement | undefined): void {
    if (target && activeImage) drawPetIcon(target, activeImage);
  }

  $effect(() => { action; frame; activeImage; draw(); });
  $effect(() => { activeImage; paintPortrait(iconCanvas); });
  $effect(() => { activeImage; paintPortrait(exportCanvas); });
  $effect(() => {
    const stored = doc.current.package;
    const key = stored ? `${stored.id}\0${stored.mimeType}` : "";
    if (key === loadedPackageKey) return;
    loadedPackageKey = key;
    const generation = ++loadGeneration;
    if (stored) void loadStored(stored.id, stored.mimeType, generation);
    else void loadStarter(generation);
  });

  onMount(() => {
    const media = window.matchMedia("(prefers-reduced-motion: reduce)");
    const syncMotion = () => {
      reducedMotion = media.matches;
      if (reducedMotion) { window.clearTimeout(frameTimer); window.clearTimeout(ambientTimer); action = "idle"; frame = 0; draw(); }
      else { play("idle", Number.POSITIVE_INFINITY); scheduleAmbient(); }
    };
    const visibility = () => { if (document.hidden) { window.clearTimeout(ambientTimer); play("idle", Number.POSITIVE_INFINITY); } else scheduleAmbient(); };
    syncMotion(); media.addEventListener("change", syncMotion); document.addEventListener("visibilitychange", visibility);
    return () => {
      media.removeEventListener("change", syncMotion); document.removeEventListener("visibilitychange", visibility);
      window.clearTimeout(frameTimer); window.clearTimeout(ambientTimer);
      if (imageObjectUrl) URL.revokeObjectURL(imageObjectUrl);
    };
  });
</script>

<svelte:window
  ondragenter={(event) => { if (event.dataTransfer?.types.includes("Files")) { event.preventDefault(); dragActive = true; } }}
  ondragover={(event) => { if (event.dataTransfer?.types.includes("Files")) event.preventDefault(); }}
  ondragleave={(event) => { if (!event.relatedTarget) dragActive = false; }}
  ondrop={(event) => { event.preventDefault(); dragActive = false; void importPackage(event.dataTransfer?.files[0]); }}
/>

<Slop>
  <main class="pet-shell" aria-label={`${activePet.displayName}, animated desktop pet`} data-slop-selection="none">
    <div class="pet-sprite" role="img" aria-label={activePet.description}>
      <canvas bind:this={canvas} width={CELL_WIDTH} height={CELL_HEIGHT} aria-hidden="true"></canvas>
    </div>
    {#if dragActive}<div class="pet-drop" data-slop-export="hide">DROP PET ZIP</div>{/if}
    {#if error}<button class="pet-error" data-slop-export="hide" onclick={() => error = null} aria-label="Dismiss error">{error}</button>{/if}
    <div class="pet-rail" data-slop-export="hide">
      <a class="pet-market" href="https://codex-pets.net/" target="_blank" rel="noreferrer">Find more pets ↗</a>
      <Button.Root type="button" class="pet-zip" onclick={() => fileInput.click()} disabled={importing} aria-label="Import pet ZIP">{importing ? "Importing…" : "Import ZIP"}</Button.Root>
      {#if doc.current.package}<Button.Root type="button" class="pet-bubu" onclick={useStarter}>Restore Bubu</Button.Root>{/if}
    </div>
    <input bind:this={fileInput} class="pet-file" type="file" tabindex="-1" aria-label="Pet ZIP file" accept=".zip,.codex-pet.zip,application/zip" onchange={event => { void importPackage(event.currentTarget.files?.[0]); event.currentTarget.value = ""; }} />
  </main>

  {#snippet icon()}
    <div class="pet-icon" aria-hidden="true">
      <canvas bind:this={iconCanvas} class="pet-icon-canvas" width="512" height="512"></canvas>
    </div>
  {/snippet}
  {#snippet exportView()}
    <div class="pet-icon" aria-label="Exported Codex Pet">
      <canvas bind:this={exportCanvas} class="pet-icon-canvas" width="512" height="512"></canvas>
    </div>
  {/snippet}
</Slop>
