<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import type { CaptureProps } from "hitslop/svelte";
  import { previewText } from "../_shared/preview";
  import BookOpen from "@lucide/svelte/icons/book-open";
  import Star from "@lucide/svelte/icons/star";
  import type schema from "./schema";
  import { readingView } from "./books";
  let { snapshot, preview = false }: CaptureProps<typeof schema> & { preview?: boolean } = $props();
  const STARS = [1, 2, 3, 4, 5] as const;
  const view = $derived(readingView(snapshot));
</script>

<article class="exportCard" aria-label="Exported reading list">
      <header class="header">
        <div class="identity">
          <h1 class="masthead">Reading list</h1>
          <div class="memberRow">
            <span class="memberName">{snapshot.memberName.trim() || "Cardholder"}</span>
            {#if snapshot.memberName.trim() && snapshot.memberSince.trim()}<span aria-hidden="true">·</span>{/if}
            <span class="memberSince">{snapshot.memberSince}</span>
          </div>
        </div>
        <div class="finishedCount" aria-label="{view.read} books finished"><strong>{view.read}</strong><span>finished</span></div>
      </header>

      <div class="ledger">
        <div class="head" aria-hidden="true">
          <span>Title</span>
          <span>Author</span>
          <span>Rating</span>
          <span>Status</span>
        </div>
        <ul class="list" aria-label="Reading list">
          {#each (preview ? (snapshot.books).slice(0, 4) : snapshot.books) as book (book.$id)}
            <li class="row exportRow">
              <span class="bookSpine" data-status={book.status} aria-hidden="true"><BookOpen size={19} strokeWidth={1.5} /></span>
              <div class="bookInfo">
                <span class="titleCell">{book.title.trim() || "Untitled book"}</span>
                <span class="authorCell">{book.author.trim() || "Unknown Author"}</span>
              </div>
              <span class="stars" aria-label="{book.rating} of 5 stars">
                {#each STARS as star}
                  <span class="star" data-filled={star <= book.rating} aria-hidden="true"><Star size={15} fill={star <= book.rating ? "currentColor" : "none"} strokeWidth={1.6} /></span>
                {/each}
              </span>
              <span class="statusPill" data-status={book.status}>{book.status || "To Read"}</span>
              {#if book.notes?.trim()}<div class="exportNotes"><strong>Notes</strong><p>{previewText(book.notes, preview, 100)}</p></div>{/if}
            </li>
          {:else}
            <li class="empty"><h2>Your next chapter starts here.</h2></li>
          {/each}
        </ul>
      </div>

      <footer class="foot">
        <span>{view.read} of {view.total} books finished</span>
        <span>One book at a time.</span>
      </footer>
    </article>

<style>
/* Explicit placement also works in the native capture renderer. */
.masthead { font-size: 44px; }
.bookSpine { grid-column: 1; grid-row: 1 / span 2; }
.bookInfo { grid-column: 2; grid-row: 1; }
.stars { grid-column: 2; grid-row: 2; }
.statusPill { grid-column: 3; grid-row: 1 / span 2; }
.exportNotes { grid-column: 2 / 4; grid-row: 3; }
</style>
