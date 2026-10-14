<script lang="ts">
  import "hitslop/base.css";
  import "./styles.css";
  import type { CaptureProps } from "hitslop/svelte";
  import type schema from "./schema";
  import { displayDate, journalView } from "./journal";
  import JournalPage from "./StaticJournalPage.svelte";

  let { snapshot }: CaptureProps<typeof schema> = $props();
  const { hasThought } = $derived(journalView(snapshot));
</script>

<article class={"fmj-exportBook fmj-book"} aria-label="Exported five minute journal">
  <header class={"fmj-masthead"}>
    <h1 class={"fmj-title"}>Five Minute Journal</h1>
    <p class={"fmj-exportDate"}>{displayDate(snapshot.date)}</p>
  </header>
  <JournalPage period="am" {snapshot} />
  <JournalPage period="pm" {snapshot} />
  {#if hasThought}
    <blockquote class={"fmj-exportThought"}>
      {#if snapshot.quote.trim()}<p class={"fmj-quoteText"}>{snapshot.quote}</p>{/if}
      {#if snapshot.quoteAuthor.trim()}<footer class={"fmj-authorText"}>— {snapshot.quoteAuthor}</footer>{/if}
    </blockquote>
  {/if}
</article>
