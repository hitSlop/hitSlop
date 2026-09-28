<script lang="ts">
  import { useDocument } from "@hitslop/document/svelte";
  import schema from "./schema";
  import { displayDate } from "./journal";
  import JournalPage from "./JournalPage.svelte";

  const doc = useDocument(schema);
  const data = $derived(doc.current);
</script>

<article class={"fmj-exportBook fmj-book"} aria-label="Exported five minute journal">
  <header class={"fmj-masthead"}>
    <h1 class={"fmj-title"}>Five Minute Journal</h1>
    <p class={"fmj-exportDate"}>{displayDate(data.date)}</p>
  </header>
  <JournalPage period="am" staticView />
  <JournalPage period="pm" staticView />
  {#if data.quote.trim() || data.quoteAuthor.trim()}
    <blockquote class={"fmj-exportThought"}>
      {#if data.quote.trim()}<p class={"fmj-quoteText"}>{data.quote}</p>{/if}
      {#if data.quoteAuthor.trim()}<footer class={"fmj-authorText"}>— {data.quoteAuthor}</footer>{/if}
    </blockquote>
  {/if}
</article>
