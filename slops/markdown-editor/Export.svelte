<script lang="ts">
  import SvelteMarkdown from "@humanspeak/svelte-markdown";
  import doc from "./schema";
  import { markdownView } from "./markdown";

  const { content } = $derived(doc.current);
  const view = $derived(markdownView(doc.current));
</script>

<article class={"md-exportSheet"} aria-label="Exported manuscript {view.title}">
  <header>
    <p class={"md-exportEyebrow"}>Manuscript</p>
    <h1 class={"md-exportTitle"}>{view.title}</h1>
    <p class={"md-exportMeta"}>{view.words} words · {view.readingTime} min read</p>
  </header>
  <div class={`md-prose md-exportProse`}>
    {#if !view.blank}
      <SvelteMarkdown source={content} />
    {:else}
      <p class={"md-empty"}>This page is blank.</p>
    {/if}
  </div>
</article>
