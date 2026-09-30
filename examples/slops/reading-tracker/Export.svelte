<script lang="ts">
import { useDocument } from "@hitslop/document/svelte";
import BookOpen from "@lucide/svelte/icons/book-open";
import Star from "@lucide/svelte/icons/star";
import schema from "./schema";
const STARS = [1, 2, 3, 4, 5] as const;
const doc = useDocument(schema);
const totalBooks = $derived(doc.current.books.length);
const readBooks = $derived(doc.current.books.filter((book) => book.status === "Read").length);
</script>

<article class="exportCard" aria-label="Exported reading list">
      <header class="header">
        <div class="identity">
          <h1 class="masthead">Reading list</h1>
          <div class="memberRow">
            <span class="memberName">{doc.current.memberName.trim() || "Cardholder"}</span>
            {#if doc.current.memberName.trim() && doc.current.memberSince.trim()}<span aria-hidden="true">·</span>{/if}
            <span class="memberSince">{doc.current.memberSince}</span>
          </div>
        </div>
        <div class="finishedCount" aria-label="{readBooks} books finished"><strong>{readBooks}</strong><span>finished</span></div>
      </header>

      <div class="ledger">
        <div class="head" aria-hidden="true">
          <span>Title</span>
          <span>Author</span>
          <span>Rating</span>
          <span>Status</span>
        </div>
        <ul class="list" aria-label="Reading list">
          {#each doc.current.books as book (book.$id)}
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
              {#if book.notes?.trim()}<div class="exportNotes"><strong>Notes</strong><p>{book.notes}</p></div>{/if}
            </li>
          {:else}
            <li class="empty"><h2>Your next chapter starts here.</h2></li>
          {/each}
        </ul>
      </div>

      <footer class="foot">
        <span>{readBooks} of {totalBooks} books finished</span>
        <span>One book at a time.</span>
      </footer>
    </article>
