<script lang="ts">
  import { previewText } from "../_shared/preview";
  let { preview = false }: { preview?: boolean } = $props();
  // The export renders saved notes only. It never imports the recorder and never asks
  // for the microphone; recordings are listed, not played.
  import doc from "./schema";
  import { durationLabel, padView, sizeLabel } from "./notes";
  import NoteMeta from "./NoteMeta.svelte";
  import Spiral from "./Spiral.svelte";

  const view = $derived(padView(doc.current));
</script>

<main class="vn vn-export">
  <section class="vn-pad" aria-label="Notes">
    <Spiral rings={13} />
    <div class="vn-pad-scroll">
      <header class="vn-pad-head">
        <h1 class="vn-title">{doc.current.title || "Voice notes"}</h1>
        <p class="vn-summary">
          {view.count} {view.count === 1 ? "note" : "notes"} · {durationLabel(view.recordedMs)} recorded · {view.words} words
        </p>
      </header>
      <ol class="vn-notes">
        {#each (preview ? (view.notes).slice(0, 3) : view.notes) as note (note.$id)}
          <li class="vn-note">
            <div class="vn-margin">
              <span class="vn-export-mark" data-kind={note.clip ? "clip" : "typed"} aria-hidden="true"></span>
            </div>
            <div class="vn-body">
              <div class="vn-note-head"><NoteMeta {note} /></div>
              <p class="vn-transcript vn-transcript-static" data-empty={!note.transcript.trim()}>{previewText(note.transcript.trim() || "No transcript.", preview, 220)}</p>
              {#if note.clip}
                <p class="vn-status vn-status-quiet">
                  Recording kept in this pad · {sizeLabel(note.clip.byteLength)}{#if note.transcribed} · transcribed on-device <span class="vn-nowrap">{note.transcribed.locale}</span>{/if}
                </p>
              {/if}
            </div>
          </li>
        {:else}
          <li class="vn-empty"><p>The pad is clear.</p></li>
        {/each}
      </ol>
    </div>
  </section>
</main>
