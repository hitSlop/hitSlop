<script lang="ts">
  import type { CaptureProps, WidgetProps } from "hitslop/svelte";
  import BookOpen from "@lucide/svelte/icons/book-open";
  import type schema from "./schema";
  import { readingView } from "./books";

  let { snapshot, family, width, height }: CaptureProps<typeof schema> & WidgetProps = $props();
  const view = $derived(readingView(snapshot));
  const reading = $derived(snapshot.books.filter(book => book.status === "Reading"));
  const queued = $derived(snapshot.books.filter(book => book.status === "To Read"));
  const featured = $derived(reading[0] ?? queued[0] ?? snapshot.books.find(book => book.status === "Read"));
  const nextBooks = $derived([...reading.slice(1), ...queued].filter(book => book.$id !== featured?.$id).slice(0, 3));
  const label = $derived(featured?.status === "Reading" ? "On your nightstand" : featured?.status === "To Read" ? "Your next chapter" : "From your shelf");
  const shorten = (text: string, limit: number) => {
    const letters = Array.from(text.trim());
    return letters.length > limit ? `${letters.slice(0, limit - 1).join("")}…` : letters.join("");
  };
</script>

<main class="reading-widget" data-family={family} style:width={`${width}px`} style:height={`${height}px`} aria-label="Reading tracker widget">
  <header><h1>Reading list</h1><span class="reading-finished"><strong>{view.read}</strong> finished</span></header>
  {#if featured}
    <section class="reading-feature">
      {#if family !== "small"}<div class="reading-cover" aria-hidden="true"><BookOpen size={34} strokeWidth={1.3} /></div>{/if}
      <div class="reading-book">
        <p class="reading-label">{label}</p>
        <h2>{shorten(featured.title || "Untitled book", family === "small" ? 28 : 48)}</h2>
        <p class="reading-author">{shorten(featured.author || "Unknown author", family === "small" ? 24 : 36)}</p>
      </div>
    </section>
  {:else}
    <section class="reading-empty"><BookOpen size={26} strokeWidth={1.4} /><h2>{family === "small" ? "Your next chapter." : "Your next chapter starts here."}</h2></section>
  {/if}
  {#if family === "large"}
    <section class="reading-queue" aria-label="More on your reading list">
      <p class="reading-label">More on your list</p>
      {#each nextBooks as book (book.$id)}
        <div class="reading-row"><span class="reading-spine" data-reading={book.status === "Reading"} aria-hidden="true"></span><div><h3>{book.title.trim() || "Untitled book"}</h3><p>{book.author.trim() || "Unknown author"}</p></div><span class="reading-status">{book.status === "Reading" ? "Reading" : "Up next"}</span></div>
      {:else}
        <p class="reading-rest">{view.total ? "One book at a time." : "Add a book to begin your shelf."}</p>
      {/each}
    </section>
  {/if}
  <footer><span>{shorten(snapshot.memberName || "Personal library", family === "small" ? 20 : 32)}</span>{#if family !== "small"}<span>{view.total} {view.total === 1 ? "book" : "books"}</span>{/if}</footer>
</main>

<style>
  .reading-widget { box-sizing: border-box; display: flex; flex-direction: column; gap: 13px; padding: 16px; overflow: hidden; background: var(--slop-paper); color: var(--slop-ink); font-family: var(--font); }
  header { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding-bottom: 10px; border-bottom: 1px solid var(--slop-border); flex-shrink: 0; }
  h1 { margin: 0; color: var(--slop-stamp); font-family: var(--headingFont); font-size: 27px; line-height: 1; font-weight: 550; letter-spacing: -0.04em; }
  .reading-finished { display: flex; align-items: baseline; gap: 5px; color: var(--slop-stamp); font-size: 10px; white-space: nowrap; }
  .reading-finished strong { font-family: var(--headingFont); font-size: 23px; font-weight: 550; }
  .reading-feature { display: flex; align-items: center; gap: 14px; min-height: 0; flex: 1; overflow: hidden; }
  .reading-cover { width: 58px; height: 76px; display: flex; align-items: center; justify-content: center; flex-shrink: 0; border-radius: 2px 8px 8px 2px; border-left: 5px solid var(--slop-reading); background: var(--slop-readingSoft); color: var(--slop-reading); }
  .reading-book { display: flex; flex-direction: column; gap: 3px; min-width: 0; height: 82px; flex: 1; overflow: hidden; }
  .reading-book .reading-label { height: 11px; line-height: 11px; flex-shrink: 0; }
  .reading-book h2 { height: 52px; line-height: 26px; flex-shrink: 0; }
  .reading-book .reading-author { height: 13px; line-height: 13px; flex-shrink: 0; }
  .reading-label { margin: 0; color: var(--slop-muted); font-size: 9px; line-height: 1.2; font-weight: 700; text-transform: uppercase; letter-spacing: 0.07em; }
  h2 { margin: 0; font-family: var(--headingFont); font-size: 25px; font-weight: 550; letter-spacing: -0.02em; line-height: 1.05; overflow: hidden; overflow-wrap: anywhere; max-height: 54px; }
  .reading-author { margin: 0; font-size: 11px; color: var(--slop-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .reading-queue { flex-shrink: 0; }
  .reading-queue > .reading-label { margin-bottom: 5px; }
  .reading-row { display: flex; align-items: center; gap: 9px; height: 43px; border-top: 1px solid var(--slop-rule); }
  .reading-spine { width: 6px; height: 26px; flex-shrink: 0; background: var(--slop-stamp); border-radius: 1px; }
  .reading-spine[data-reading="true"] { background: var(--slop-reading); }
  .reading-row > div { flex: 1; min-width: 0; }
  h3 { margin: 0; font-family: var(--headingFont); font-weight: 550; font-size: 16px; line-height: 1.2; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .reading-row p { margin: 2px 0 0; font-size: 10px; color: var(--slop-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .reading-status { flex-shrink: 0; color: var(--slop-muted); font-size: 9px; }
  .reading-rest { margin: 14px 0; color: var(--slop-muted); font-size: 12px; }
  footer { display: flex; justify-content: space-between; gap: 10px; flex-shrink: 0; margin-top: auto; color: var(--slop-muted); font-size: 9px; line-height: 1.2; }
  footer span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .reading-empty { display: flex; align-items: center; gap: 12px; min-height: 0; overflow: hidden; flex: 1; color: var(--slop-stamp); }
  .reading-empty h2 { font-size: 23px; max-height: none; }
  [data-family="small"] { padding: 13px; gap: 9px; }
  [data-family="small"] header { gap: 4px; padding-bottom: 7px; }
  [data-family="small"] h1 { font-size: 21px; }
  [data-family="small"] .reading-finished { flex-direction: column; align-items: center; font-size: 7px; gap: 1px; }
  [data-family="small"] .reading-finished strong { font-size: 17px; line-height: 1; }
  [data-family="small"] h2 { font-size: 22px; max-height: 47px; }
  [data-family="small"] .reading-book { height: 74px; gap: 4px; }
  [data-family="small"] .reading-book h2 { height: 44px; line-height: 22px; }
  [data-family="small"] .reading-book .reading-label { height: 10px; line-height: 10px; }
  [data-family="small"] .reading-book .reading-author { height: 12px; line-height: 12px; }
  [data-family="small"] .reading-label { font-size: 8px; letter-spacing: 0.03em; }
  [data-family="small"] .reading-author { font-size: 10px; }
  [data-family="small"] .reading-empty { flex-direction: column; align-items: flex-start; justify-content: center; gap: 5px; }
  [data-family="small"] .reading-empty h2 { font-size: 20px; }
  [data-family="medium"] { gap: 9px; padding: 13px 16px; }
  [data-family="medium"] header { padding-bottom: 7px; }
  [data-family="medium"] h1 { font-size: 25px; }
  [data-family="medium"] .reading-cover { width: 45px; height: 62px; }
  [data-family="medium"] h2 { font-size: 23px; max-height: 48px; }
  [data-family="medium"] .reading-book { height: 76px; }
  [data-family="medium"] .reading-book h2 { height: 46px; line-height: 23px; }
  [data-family="large"] { gap: 10px; }
</style>
