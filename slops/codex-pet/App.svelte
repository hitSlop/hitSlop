<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { attachments, getDocument } from "hitslop/svelte";
  import { onMount, tick } from "svelte";
  import { Button } from "bits-ui";
  import license from "./assets/BUBU-LICENSE.txt?raw";
  import { parsePetPackage } from "./pet-package";
  import { ACTIONS, CELL_HEIGHT, CELL_WIDTH, STARTER, adoptPackage, nextAmbient, petView, restoreStarter, type PetAction, type StoredPackage } from "./pet";
  import { loadPet } from "./sheet";
  import schema from "./schema";

  const doc = getDocument(schema);
  const view = $derived(petView(doc.current));
  let canvas: HTMLCanvasElement;
  let fileInput: HTMLInputElement;
  // The decoded sheet, the playing action and timers are view state; only the package is saved.
  let image = $state.raw<HTMLImageElement | null>(null);
  let activePet = $state.raw(STARTER);
  let action = $state<PetAction>("idle");
  let frame = $state(0);
  let error = $state<string | null>(null);
  let dragActive = $state(false);
  let importing = $state(false);
  let reducedMotion = $state(false);
  let release = () => {};
  let frameTimer = 0, ambientTimer = 0, loadGeneration = 0;
  let loadedKey: string | null = null;

  function draw(): void {
    if (!canvas || !image) return;
    const context = canvas.getContext("2d");
    if (!context) return;
    context.clearRect(0, 0, CELL_WIDTH, CELL_HEIGHT);
    const spec = ACTIONS[action];
    context.drawImage(image, frame * CELL_WIDTH, spec.row * CELL_HEIGHT, CELL_WIDTH, CELL_HEIGHT, 0, 0, CELL_WIDTH, CELL_HEIGHT);
  }

  function play(next: PetAction, cycles = 1): void {
    window.clearTimeout(frameTimer);
    action = next; frame = 0; draw();
    if (reducedMotion) return;
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

  function scheduleAmbient(): void {
    window.clearTimeout(ambientTimer);
    if (reducedMotion || document.hidden) return;
    const next = nextAmbient(new Date().getHours(), Math.random);
    ambientTimer = window.setTimeout(() => { play(next.action, 1); scheduleAmbient(); }, next.delay);
  }

  /** Shows the stored pet (or Bubu). Loading never writes: the name was saved with the package. */
  async function show(stored: StoredPackage | undefined, generation: number): Promise<void> {
    const loaded = await loadPet(stored);
    if (generation !== loadGeneration) { loaded.release(); return; }
    release(); release = loaded.release;
    activePet = loaded.pet; image = loaded.image; error = loaded.error;
    await tick();
    if (generation !== loadGeneration) return;
    play("idle", Number.POSITIVE_INFINITY);
    scheduleAmbient();
  }

  async function importPackage(file: File | undefined): Promise<void> {
    if (!file || importing) return;
    importing = true; error = null;
    try {
      const parsed = await parsePetPackage(file);
      await attachments.import<typeof schema.descriptor>(file, (tx, ref) => adoptPackage(tx, ref, parsed.displayName));
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally { importing = false; }
  }

  function useStarter(): void {
    if (!view.imported) return;
    doc.change((tx) => restoreStarter(tx, doc.current));
    error = null;
  }

  $effect(() => { action; frame; image; draw(); });
  $effect(() => {
    const key = view.key;
    if (key === loadedKey) return;
    loadedKey = key;
    void show(view.stored, ++loadGeneration);
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
      loadGeneration += 1;
      release();
    };
  });
</script>

<svelte:window
  ondragenter={(event) => { if (event.dataTransfer?.types.includes("Files")) { event.preventDefault(); dragActive = true; } }}
  ondragover={(event) => { if (event.dataTransfer?.types.includes("Files")) event.preventDefault(); }}
  ondragleave={(event) => { if (!event.relatedTarget) dragActive = false; }}
  ondrop={(event) => { event.preventDefault(); dragActive = false; void importPackage(event.dataTransfer?.files[0]); }}
/>

<main class="pet-shell" aria-label={`${activePet.displayName}, animated desktop pet`} data-starter-license={license}>
  <div class="pet-sprite" role="img" aria-label={activePet.description}>
    <canvas bind:this={canvas} width={CELL_WIDTH} height={CELL_HEIGHT} aria-hidden="true"></canvas>
  </div>
  {#if dragActive}<div class="pet-drop" data-slop-export="hide">DROP PET ZIP</div>{/if}
  {#if error}<button class="pet-error" data-slop-export="hide" onclick={() => error = null} aria-label="Dismiss error">{error}</button>{/if}
  <div class="pet-rail" data-slop-export="hide">
    <a class="pet-market" href="https://codex-pets.net/" target="_blank" rel="noreferrer">Find more pets ↗</a>
    <Button.Root type="button" class="pet-zip" onclick={() => fileInput.click()} disabled={importing} aria-label="Import pet ZIP">{importing ? "Importing…" : "Import ZIP"}</Button.Root>
    {#if view.imported}<Button.Root type="button" class="pet-bubu" onclick={useStarter}>Restore Bubu</Button.Root>{/if}
  </div>
  <input bind:this={fileInput} class="pet-file" type="file" tabindex="-1" aria-label="Pet ZIP file" accept=".zip,.codex-pet.zip,application/zip" onchange={event => { void importPackage(event.currentTarget.files?.[0]); event.currentTarget.value = ""; }} />
</main>
