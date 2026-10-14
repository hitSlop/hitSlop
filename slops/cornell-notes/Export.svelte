<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import type { CaptureProps } from "hitslop/svelte";
  import { previewText } from "../_shared/preview";
  import type schema from "./schema";
  import { notesView } from "./notes";
  let { snapshot, preview = false }: CaptureProps<typeof schema> & { preview?: boolean } = $props();
  const view = $derived(notesView(snapshot));
</script>

<main class="cornell-page" class:theme-dark={view.dark}>
  <div class="notebook-sheet">
    <header class="sheet-header">
      <div class="header-top-bar">
        <div class="course-meta">
          <div class="course-badge"><span class="course-input">{snapshot.course}</span></div>
          <span class="meta-sep">·</span>
          <div class="meta-item"><span class="meta-label">INSTRUCTOR</span><span class="meta-input">{snapshot.lecturer}</span></div>
          <span class="meta-sep">·</span>
          <strong class="date-text">{snapshot.date}</strong>
        </div>
      </div>
      <div class="topic-row"><h1 class="topic-input">{snapshot.topic}</h1></div>
    </header>
    <div class="cornell-grid">
      <div class="items-stream">
        {#each (preview ? (view.items).slice(0, 3) : view.items) as item, index (item.$id)}
          <div class="cornell-entry">
            <div class="cue-cell"><span class="cue-prefix">Q{index + 1}</span><p class="cue-textarea">{previewText(item.cue, preview, 80)}</p></div>
            <div class="margin-rule"></div>
            <div class="notes-cell"><p class="notes-textarea">{previewText(item.notes, preview, 200)}</p></div>
          </div>
        {/each}
      </div>
    </div>
    <footer class="summary-box"><p class="summary-textarea">{previewText(snapshot.summary, preview, 180)}</p></footer>
  </div>
</main>
