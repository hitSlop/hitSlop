<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import { attachments, bindText, getDocument } from "hitslop/svelte";
  import { Button } from "bits-ui";
  import ImagePlus from "@lucide/svelte/icons/image-plus";
  import X from "@lucide/svelte/icons/x";
  import schema, { maxCaption, maxTiles } from "./schema";
  import { acceptedTypes, addTile, boardView, captionFromName, fileProblem, removeTile, setCaption, type Tile } from "./tiles";
  import Scene from "./motion-core/InfiniteGalleryScene.svelte";
  import Magnetic from "./motion-core/Magnetic.svelte";

  const doc = getDocument(schema);
  let input: HTMLInputElement;
  let notice = $state("");
  let importing = $state(false);
  let dragging = $state(false);

  const view = $derived(boardView(doc.current));
  // The tunnel reloads its textures only when the set of images changes, not on a caption edit.
  const sources = $derived(view.tiles.map((tile) => attachments.url(tile.image.id)).join("\n"));
  const images = $derived(sources ? sources.split("\n") : []);

  async function addFiles(files: Iterable<File>) {
    importing = true;
    notice = "";
    try {
      for (const file of files) {
        if (doc.current.tiles.length >= maxTiles) { notice = `The board holds ${maxTiles} images.`; break; }
        const problem = fileProblem(file);
        if (problem) { notice = problem; continue; }
        await attachments.import<typeof schema.descriptor>(file, (tx, ref) => {
          addTile(tx, doc.current, ref, captionFromName(file.name));
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
  function caption(tile: Tile, value: string) {
    void doc.change((tx) => setCaption(tx, doc.current, tile, value));
  }
  function remove(tile: Tile) {
    void doc.change((tx) => removeTile(tx, doc.current, tile));
  }
</script>

<main
  class="wall"
  data-dragging={dragging}
  aria-label="Moodboard"
  ondragover={(event) => { if (event.dataTransfer?.types.includes("Files")) { event.preventDefault(); dragging = true; } }}
  ondragleave={() => (dragging = false)}
  ondrop={(event) => { event.preventDefault(); dragging = false; void addFiles(event.dataTransfer?.files ?? []); }}
>
  <header class="head">
    <input class="title" aria-label="Board title" placeholder="Moodboard" use:bindText={doc.fields.title} />
    <p class="count">{view.countLabel}</p>
  </header>

  <section class="stage" aria-label="Image tunnel">
    {#if images.length}
      <Scene {images} speed={1} visibleCount={Math.min(8, Math.max(4, images.length * 2))} />
      <p class="hint" aria-hidden="true">Scroll, drag or use the arrow keys</p>
    {:else}
      <div class="empty">
        <h2>Start your board</h2>
        <p>Drop JPEG, PNG or WebP images here, or add them with the button.</p>
      </div>
    {/if}
  </section>

  {#if notice}<p class="notice" role="status">{notice}</p>{/if}

  <section class="strip" aria-label="Images on this board">
    <input bind:this={input} class="sr-only" tabindex="-1" type="file" multiple accept={acceptedTypes.join(",")} aria-label="Add images" onchange={picked} />
    <Magnetic class="magnet">
      <Button.Root class="add" disabled={importing || view.full} onclick={() => input.click()}><ImagePlus size={20} />{importing ? "Adding…" : "Add images"}</Button.Root>
    </Magnetic>
    <ul>
      {#each view.tiles as tile (tile.$id)}
        <li>
          <img src={attachments.url(tile.image.id)} alt={tile.caption || "Board image"} onerror={() => (notice = "One image couldn’t be opened.")} />
          <input aria-label="Caption" maxlength={maxCaption} value={tile.caption} placeholder="Caption" onchange={(event) => caption(tile, event.currentTarget.value)} />
          <button class="x" aria-label={`Remove ${tile.caption || "image"}`} onclick={() => remove(tile)}><X size={14} /></button>
        </li>
      {/each}
    </ul>
  </section>
</main>
