<script lang="ts">
  import doc from "./schema";
</script>

<main class="cornell-page" class:theme-dark={doc.current.theme === "dark"}>
  <div class="notebook-sheet">
    <header class="sheet-header">
      <div class="header-top-bar">
        <div class="course-meta">
          <div class="course-badge"><span class="course-input">{doc.current.course}</span></div>
          <span class="meta-sep">·</span>
          <div class="meta-item"><span class="meta-label">INSTRUCTOR</span><span class="meta-input">{doc.current.lecturer}</span></div>
          <span class="meta-sep">·</span>
          <strong class="date-text">{doc.current.date}</strong>
        </div>
      </div>
      <div class="topic-row"><h1 class="topic-input">{doc.current.topic}</h1></div>
    </header>
    <div class="cornell-grid">
      <div class="items-stream">
        {#each doc.current.items as item, index (item.$id)}
          <div class="cornell-entry">
            <div class="cue-cell"><span class="cue-prefix">Q{index + 1}</span><p class="cue-textarea">{item.cue}</p></div>
            <div class="margin-rule"></div>
            <div class="notes-cell"><p class="notes-textarea">{item.notes}</p></div>
          </div>
        {/each}
      </div>
    </div>
    <footer class="summary-box"><p class="summary-textarea">{doc.current.summary}</p></footer>
  </div>
</main>
