<script lang="ts">
  // A fresh, read-only page: it renders saved frames only and never asks for the camera.
  import doc from "./schema";
  import { dayKey, dayLabel, frameView, shortLabel, sizeLabel } from "./frames";
  import Photo from "./Photo.svelte";

  let { mode = "export" }: { mode?: "preview" | "export" } = $props();
  const today = dayKey(new Date());
  const view = $derived(frameView(doc.current, today));
  const hero = $derived(view.latest);
  // The most recent frames, oldest first, read like the flipbook.
  const limit = $derived(mode === "preview" ? 8 : view.stack.length);
  const sheet = $derived(view.stack.slice(-limit));
  const hidden = $derived(view.stack.length - sheet.length);

</script>

<article class="df-export">
  <header class="df-head">
    <div class="df-heading">
      <p class="df-eyebrow">Daily Frame{view.sample ? " · sample" : ""}</p>
      <h1 class="df-title">{doc.current.title || "Daily Frame"}</h1>
    </div>
    <dl class="df-readout">
      <div><dt>frames</dt><dd>{view.stack.length}</dd></div>
      <div><dt>day run</dt><dd>{view.streak}</dd></div>
    </dl>
  </header>

  {#if hero}
    <figure class="df-print df-export-hero">
      <div class="df-window"><Photo shown={hero} /></div>
      <figcaption class="df-caption">
        <span class="df-caption-note">{hero.note || (view.sample ? "Sample frame" : " ")}</span>
        <span class="df-caption-day">{hero.date === today ? "Today" : dayLabel(hero.date)}</span>
      </figcaption>
    </figure>
  {/if}

  <p class="df-strip-head">
    {#if view.sample}<span class="df-sample-tag">Sample</span> drawn frames · no photos kept yet
    {:else}{view.frames.length} {view.frames.length === 1 ? "frame" : "frames"} · {shortLabel(view.first?.date ?? today)} to {shortLabel(view.latest?.date ?? today)} · {sizeLabel(view.bytes)}{/if}
  </p>
  <ol class="df-sheet">
    {#each sheet as shown (shown.key)}
      <li class="df-sheet-print">
        <span class="df-sheet-photo"><Photo {shown} /></span>
        <span class="df-sheet-note">{shown.note || dayLabel(shown.date)}</span>
      </li>
    {/each}
  </ol>
  {#if hidden > 0}<p class="df-sheet-more">and {hidden} earlier {hidden === 1 ? "frame" : "frames"} in the document</p>{/if}
</article>
