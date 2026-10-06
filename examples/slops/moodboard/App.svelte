<script lang="ts">
  import { untrack, onMount, tick } from "svelte";
  import { attachments, bindText, capture } from "@hitslop/document/svelte";
  import { Button } from "bits-ui";
  import ImagePlus from "@lucide/svelte/icons/image-plus";
  import X from "@lucide/svelte/icons/x";
  import doc, { maxTiles } from "./schema";
  import Scene from "./motion-core/InfiniteGalleryScene.svelte";
  import Magnetic from "./motion-core/Magnetic.svelte";
  import { ui } from "./ui.svelte";

  const accepted = ["image/jpeg", "image/png", "image/webp"];
  let input: HTMLInputElement;
  let notice = $state("");
  let importing = $state(false);
  let dragging = $state(false);
  const pending = new Set<Promise<void>>();

  const tiles = $derived(doc.current.tiles);
  const full = $derived(tiles.length >= maxTiles);
  const images = $derived(tiles.flatMap((tile) => (ui.urls[tile.image.id] ? [{ src: ui.urls[tile.image.id]!, alt: tile.caption }] : [])));

  // Read each image once, keep its object URL while it is on the board, and revoke it when it leaves.
  $effect(() => {
    const refs = tiles.map((tile) => tile.image);
    untrack(() => {
      const wanted = new Set(refs.map((ref) => ref.id));
      for (const id of Object.keys(ui.urls)) {
        if (wanted.has(id)) continue;
        URL.revokeObjectURL(ui.urls[id]!);
        delete ui.urls[id];
      }
      for (const ref of refs) {
        if (ui.urls[ref.id]) continue;
        const job: Promise<void> = attachments.read(ref.id, { type: ref.mimeType })
          .then((blob) => { if (!ui.urls[ref.id]) ui.urls[ref.id] = URL.createObjectURL(blob); })
          .catch(() => { notice = "One image couldn’t be opened."; })
          .finally(() => pending.delete(job));
        pending.add(job);
      }
    });
  });

  onMount(() => {
    const stop = capture.onPrepare(async () => { await Promise.all([...pending]); await tick(); });
    return () => {
      stop();
      for (const url of Object.values(ui.urls)) URL.revokeObjectURL(url);
      ui.urls = {};
    };
  });

  async function addFiles(files: Iterable<File>) {
    importing = true;
    notice = "";
    try {
      for (const file of files) {
        if (doc.current.tiles.length >= maxTiles) { notice = `The board holds ${maxTiles} images.`; break; }
        if (!accepted.includes(file.type)) { notice = "Choose JPEG, PNG or WebP images."; continue; }
        if (file.size > 10 * 1024 * 1024) { notice = `${file.name} is over 10 MB.`; continue; }
        await attachments.import<typeof doc.descriptor>(file, (tx, ref) => {
          tx.fields.tiles.insert({ image: { id: ref.id, mimeType: file.type }, caption: file.name.replace(/\.[^.]+$/, "").slice(0, 60) });
        });
      }
    } catch (error) {
      notice = error instanceof Error ? `Couldn’t add that image. ${error.message}` : "Couldn’t add that image.";
    } finally {
      importing = false;
    }
  }
  function picked(event: Event) {
    const files = [...(event.currentTarget as HTMLInputElement).files ?? []];
    (event.currentTarget as HTMLInputElement).value = "";
    void addFiles(files);
  }
</script>

<main
  class="wall"
  data-slop-selection="none"
  data-dragging={dragging}
  aria-label="Moodboard"
  ondragover={(event) => { if (event.dataTransfer?.types.includes("Files")) { event.preventDefault(); dragging = true; } }}
  ondragleave={() => (dragging = false)}
  ondrop={(event) => { event.preventDefault(); dragging = false; void addFiles(event.dataTransfer?.files ?? []); }}
>
  <header class="head">
    <input class="title" aria-label="Board title" placeholder="Moodboard" use:bindText={doc.fields.title} />
    <p class="count">{tiles.length} {tiles.length === 1 ? "image" : "images"}</p>
  </header>

  <section class="stage" aria-label="Image tunnel">
    {#if images.length}
      <Scene {images} speed={1} visibleCount={Math.min(8, Math.max(4, images.length * 2))} />
      <p class="hint" aria-hidden="true">Scroll, drag or use the arrow keys</p>
    {:else}
      <div class="empty">
        <h2>{tiles.length ? "Opening your images…" : "Start your board"}</h2>
        <p>{tiles.length ? "One moment." : "Drop JPEG, PNG or WebP images here, or add them with the button."}</p>
      </div>
    {/if}
  </section>

  {#if notice}<p class="notice" role="status">{notice}</p>{/if}

  <section class="strip" aria-label="Images on this board">
    <input bind:this={input} class="sr-only" tabindex="-1" type="file" multiple accept={accepted.join(",")} aria-label="Add images" onchange={picked} />
    <Magnetic class="magnet">
      <Button.Root class="add" disabled={importing || full} onclick={() => input.click()}><ImagePlus size={20} />{importing ? "Adding…" : "Add images"}</Button.Root>
    </Magnetic>
    <ul>
      {#each tiles as tile (tile.$id)}
        <li>
          {#if ui.urls[tile.image.id]}<img src={ui.urls[tile.image.id]} alt={tile.caption || "Board image"} />{:else}<span class="ph"></span>{/if}
          <input aria-label="Caption" maxlength="60" value={tile.caption} placeholder="Caption" onchange={(event) => doc.at(tile).caption.set(event.currentTarget.value.trim())} />
          <button class="x" aria-label={`Remove ${tile.caption || "image"}`} onclick={() => doc.fields.tiles.remove(tile.$id)}><X size={14} /></button>
        </li>
      {/each}
    </ul>
  </section>
</main>
