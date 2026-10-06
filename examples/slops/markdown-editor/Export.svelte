<script lang="ts">
  import SvelteMarkdown from "@humanspeak/svelte-markdown";
  import doc from "./schema";

  const { title, content } = $derived(doc.current);
  const words = $derived(content.trim() === "" ? 0 : content.trim().split(/\s+/).length);
  const readingTime = $derived(Math.max(1, Math.ceil(words / 200)));
</script>

<article class={"md-exportSheet"} aria-label="Exported manuscript {title}">
  <header>
    <p class={"md-exportEyebrow"}>Manuscript</p>
    <h1 class={"md-exportTitle"}>{title.trim() || "Untitled"}</h1>
    <p class={"md-exportMeta"}>{words} words · {readingTime} min read</p>
  </header>
  <div class={`md-prose md-exportProse`}>
    {#if content.trim()}
      <SvelteMarkdown source={content} />
    {:else}
      <p class={"md-empty"}>This page is blank.</p>
    {/if}
  </div>
</article>
