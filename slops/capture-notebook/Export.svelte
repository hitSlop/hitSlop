<script lang="ts">
  // The export renders saved photos and recordings only. It never asks for a camera or
  // microphone, and it shows every entry (the editor's open photo is view state).
  import type { CaptureProps } from "hitslop/svelte";
  import { previewText } from "../_shared/preview";
  import Mic from "@lucide/svelte/icons/mic";
  import PenLine from "@lucide/svelte/icons/pen-line";
  import type schema from "./schema";
  import Print from "./Print.svelte";
  import { clock, formatBytes, formatLabel, notebookView, stamp, summary } from "./notebook";
  let { snapshot, attachmentURL, preview = false }: CaptureProps<typeof schema> & { preview?: boolean } = $props();

  const view = $derived(notebookView(snapshot));
</script>

<article class="cn-export">
  <header class="cn-export-head">
    <p class="cn-export-kicker">Capture Notebook</p>
    <h1 class="cn-title">{snapshot.title.trim() || "Untitled notebook"}</h1>
    <p class="cn-summary">{summary(view)}{view.totalBytes ? ` · ${formatBytes(view.totalBytes)} kept` : ""}</p>
  </header>

  {#if view.photos.length}
    <section class="cn-export-section" aria-label="Photos">
      <h2>Photos</h2>
      <div class="cn-export-grid">
        {#each (preview ? (view.photos).slice(0, 2) : view.photos) as entry (entry.$id)}
          {#if entry.file}
            <figure>
              <Print size="export" src={attachmentURL(entry.file.id)} alt={entry.caption.trim() || `Photo from ${stamp(entry.at)}`} width={entry.width} height={entry.height} />
              <figcaption>
                {#if entry.caption.trim()}<span class="cn-caption">{entry.caption.trim()}</span>{/if}
                <span class="cn-stamp">{stamp(entry.at)}</span>
              </figcaption>
            </figure>
          {/if}
        {/each}
      </div>
    </section>
  {/if}

  {#if view.recordings.length}
    <section class="cn-export-section" aria-label="Recordings">
      <h2>Recordings <small>{clock(view.totalSeconds)} in all</small></h2>
      <ol class="cn-entries">
        {#each (preview ? (view.recordings).slice(0, 2) : view.recordings) as entry (entry.$id)}
          <li class="cn-entry" data-kind="audio">
            <span class="cn-mark cn-mark-audio" aria-hidden="true"><Mic size={20} /></span>
            <div class="cn-entry-body">
              <p class="cn-stamp"><span>{stamp(entry.at)}</span><span>{clock(entry.seconds ?? 0)} · {formatLabel(entry)} · {formatBytes(entry.file?.byteLength ?? 0)}</span></p>
              <p class="cn-caption">{entry.caption.trim() || "Untitled recording"}</p>
            </div>
          </li>
        {/each}
      </ol>
      <p class="cn-export-hint">Recordings play in hitSlop; this page lists them.</p>
    </section>
  {/if}

  {#if view.notes.length}
    <section class="cn-export-section" aria-label="Notes">
      <h2>Notes</h2>
      <ol class="cn-entries">
        {#each (preview ? (view.notes).slice(0, 3) : view.notes) as entry (entry.$id)}
          <li class="cn-entry" data-kind="note">
            <span class="cn-mark cn-mark-note" aria-hidden="true"><PenLine size={18} /></span>
            <div class="cn-entry-body">
              <p class="cn-stamp"><span>{stamp(entry.at)}</span></p>
              <p class="cn-caption">{previewText(entry.caption.trim() || "Empty note", preview, 150)}</p>
            </div>
          </li>
        {/each}
      </ol>
    </section>
  {/if}

  {#if !view.entries.length}
    <p class="cn-export-hint">An empty page: nothing has been kept yet.</p>
  {/if}
</article>
