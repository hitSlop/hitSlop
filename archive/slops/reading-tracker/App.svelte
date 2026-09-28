<script lang="ts">
  import { Slop, bindText, useDocument } from "@hitslop/document/svelte";
  import { prefersReducedMotion } from "svelte/motion";
  import { flip } from "svelte/animate";
  import { RadioGroup, Select, ToggleGroup, Button, Dialog } from "bits-ui";
  import MessageSquareText from "@lucide/svelte/icons/message-square-text";
  import X from "@lucide/svelte/icons/x";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Check from "@lucide/svelte/icons/check";
  import BookOpen from "@lucide/svelte/icons/book-open";
  import Star from "@lucide/svelte/icons/star";
  import schema, { statuses, type Book } from "./schema";
  import { grow } from "./grow";

  type Status = (typeof statuses)[number];
  const statusItems = statuses.map((value) => ({ value, label: value }));
  const STARS = [1, 2, 3, 4, 5] as const;

  const doc = useDocument(schema);
  let newTitle = $state("");
  let newAuthor = $state("");
  let newStatus = $state<Status>("To Read");
  let notesOpen = $state(false);
  let notesBookId = $state<string | null>(null);
  let notesDraft = $state("");
  let composer = $state<HTMLInputElement>();

  const notesBook = $derived(doc.current.books.find((book) => book.$id === notesBookId));
  const totalBooks = $derived(doc.current.books.length);
  const readBooks = $derived(doc.current.books.filter((book) => book.status === "Read").length);
  const flipMs = $derived(prefersReducedMotion.current ? 0 : 220);

  function isStatus(value: string): value is Status {
    return statuses.some((status) => status === value);
  }
  function openNotes(book: Book) {
    notesBookId = book.$id;
    notesDraft = book.notes ?? "";
    notesOpen = true;
  }
  function saveNotes() {
    const book = doc.current.books.find((entry) => entry.$id === notesBookId);
    if (!book) return;
    doc.at(book).notes.set(notesDraft);
    notesOpen = false;
  }
  function addBook() {
    const title = newTitle.trim();
    if (!title) return;
    doc.fields.books.insert({
      title,
      author: newAuthor.trim() || "Unknown Author",
      rating: 0,
      status: newStatus,
    });
    newTitle = "";
    newAuthor = "";
    composer?.focus();
  }
  function setRating(book: Book, value: string) {
    const next = Number(value);
    if (next >= 1 && next <= 5) doc.at(book).rating.set(next);
  }
</script>

<Slop>
  <main class="card" data-slop-selection="none" aria-label="Personal reading list">
    <article class="pocket">
      <header class="header">
        <div class="identity">
          <h1 class="masthead">Reading list</h1>
          <div class="memberRow">
            <input class="memberName" aria-label="Reader name" use:bindText={doc.fields.memberName} placeholder="Your name" />
            <span aria-hidden="true">·</span>
            <input class="memberSince" aria-label="Reading note" use:bindText={doc.fields.memberSince} placeholder="A note about your reading" />
          </div>
        </div>
        <div class="finishedCount" aria-label="{readBooks} books finished"><strong>{readBooks}</strong><span>finished</span></div>
      </header>

      <form class="composer" data-slop-export="hide" onsubmit={(event) => { event.preventDefault(); addBook(); }}>
        <input bind:this={composer} class="titleDraft" aria-label="New book title" placeholder="Book title…" bind:value={newTitle} />
        <input class="authorDraft" aria-label="New book author" placeholder="Author…" bind:value={newAuthor} />
        <ToggleGroup.Root
          type="single"
          value={newStatus}
          onValueChange={(value) => { if (isStatus(value)) newStatus = value; }}
          class="statusGroup"
          aria-label="Status for new book"
        >
          {#each statuses as status}
            <ToggleGroup.Item value={status} class="statusStamp">{status}</ToggleGroup.Item>
          {/each}
        </ToggleGroup.Root>
        <Button.Root class="add" type="submit" aria-label="Add book" disabled={!newTitle.trim()}>
          <Plus size={13} />
          <span>Add</span>
        </Button.Root>
      </form>

      <div class="ledger">
        <div class="head" aria-hidden="true">
          <span>Title</span>
          <span>Author</span>
          <span>Rating</span>
          <span>Status</span>
          <span data-slop-export="hide"></span>
        </div>
        <ul class="list" aria-label="Reading list">
          {#each doc.current.books as book (book.$id)}
            <li class="row" data-status={book.status} animate:flip={{ duration: flipMs }}>
              <span class="bookSpine" data-status={book.status} aria-hidden="true"><BookOpen size={19} strokeWidth={1.5} /></span>
              <div class="bookInfo">
                <textarea rows="1" use:grow={book.title} class="titleCell" aria-label="Book title" use:bindText={doc.at(book).title}></textarea>
                <input class="authorCell" aria-label="Author" use:bindText={doc.at(book).author} />
              </div>
              <RadioGroup.Root
                class="stars"
                orientation="horizontal"
                value={String(book.rating)}
                onValueChange={(value) => setRating(book, value)}
                aria-label="Rating for {book.title || 'untitled book'}{book.rating ? '' : ', not yet rated'}"
              >
                {#each STARS as star}
                  <RadioGroup.Item
                    value={String(star)}
                    class="star"
                    data-filled={star <= book.rating}
                    aria-label="{star} {star === 1 ? 'star' : 'stars'}"
                  ><Star size={15} fill={star <= book.rating ? "currentColor" : "none"} strokeWidth={1.6} /></RadioGroup.Item>
                {/each}
              </RadioGroup.Root>
              <Select.Root type="single" value={book.status} items={statusItems} onValueChange={(value) => { if (isStatus(value)) doc.at(book).status.set(value); }}>
                <Select.Trigger class="statusPill" data-status={book.status} aria-label="Status for {book.title || 'untitled book'}">
                  <Select.Value placeholder="Status" />
                </Select.Trigger>
                <Select.Portal>
                  <Select.Content class="selectContent" sideOffset={6} data-slop-export="hide">
                    <Select.Viewport>
                      {#each statusItems as item (item.value)}
                        <Select.Item value={item.value} label={item.label}>
                          {#snippet children({ selected })}
                            {item.label}{#if selected}<Check size={12} strokeWidth={2.4} />{/if}
                          {/snippet}
                        </Select.Item>
                      {/each}
                    </Select.Viewport>
                  </Select.Content>
                </Select.Portal>
              </Select.Root>
              <Button.Root class="noteButton" type="button" data-slop-export="hide" aria-label="{book.notes?.trim() ? 'Edit notes' : 'Add notes'} for {book.title || 'untitled book'}" onclick={() => openNotes(book)}><MessageSquareText size={14} /><span>{book.notes?.trim() ? "Notes" : "Add note"}</span></Button.Root>
              <Button.Root class="remove" type="button" data-slop-export="hide" aria-label="Delete {book.title || 'untitled book'}" onclick={() => doc.fields.books.remove(book.$id)}>
                <Trash2 size={13} />
              </Button.Root>
            </li>
          {:else}
            <li class="empty">
              <h2>Your next chapter starts here.</h2>
              <p>Add a book you want to read, or one you’ve already loved.</p>
            </li>
          {/each}
        </ul>
      </div>

      <footer class="foot">
        <span>{readBooks} of {totalBooks} books finished</span>
        <span>One book at a time.</span>
      </footer>
    </article>
  </main>

  <Dialog.Root bind:open={notesOpen}>
    <Dialog.Portal>
      <Dialog.Overlay class="notesOverlay" data-slop-export="hide" />
      <Dialog.Content class="notesDialog" data-slop-export="hide">
        <form onsubmit={(event) => { event.preventDefault(); saveNotes(); }}>
          <header class="notesHeader">
            <Dialog.Title class="notesTitle">Notes on {notesBook?.title || "this book"}</Dialog.Title>
            <Dialog.Close class="notesClose" type="button" aria-label="Close notes"><X size={18} /></Dialog.Close>
          </header>
          <Dialog.Description class="notesDescription">Thoughts, favorite passages, or something to come back to.</Dialog.Description>
          <label class="notesLabel" for="book-notes">Your notes</label>
          <textarea id="book-notes" class="notesInput" bind:value={notesDraft} placeholder="What stayed with you?" rows="8"></textarea>
          <footer class="notesActions">
            <Dialog.Close class="notesCancel" type="button">Cancel</Dialog.Close>
            <Button.Root class="notesCancel notesSave" type="submit" disabled={!notesBook}>Save notes</Button.Root>
          </footer>
        </form>
      </Dialog.Content>
    </Dialog.Portal>
  </Dialog.Root>

  {#snippet exportView()}
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
  {/snippet}

  {#snippet icon()}
    <div class="iconSurface" aria-hidden="true">
      <svg class="iconGraphic" viewBox="0 0 512 512" fill="none">
        <rect x="28" y="28" width="456" height="456" rx="104" fill="var(--slop-stamp)"/>
        <rect x="102" y="111" width="76" height="294" rx="12" fill="#d8c9ed"/>
        <path d="M120 148H160M120 367H160" stroke="var(--slop-stamp)" stroke-width="8" stroke-linecap="round"/>
        <rect x="194" y="87" width="100" height="318" rx="12" fill="var(--slop-paper)"/>
        <path d="M225 87H263V222L244 207L225 222Z" fill="#d994b5"/>
        <rect x="308" y="144" width="72" height="261" rx="12" transform="rotate(-9 308 144)" fill="#ead7e3"/>
        <path d="M91 416H429" stroke="#d994b5" stroke-width="12" stroke-linecap="round"/>
        {#if totalBooks > 0 && readBooks === totalBooks}<path d="M219 322L242 345L274 300" stroke="var(--slop-read)" stroke-width="13" stroke-linecap="round" stroke-linejoin="round"/>{/if}
      </svg>
    </div>
  {/snippet}
</Slop>
