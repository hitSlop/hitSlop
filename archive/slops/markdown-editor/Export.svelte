<script lang="ts">
  import SvelteMarkdown from "@humanspeak/svelte-markdown";
  
  let { title, content, words, readingTime }: {
    title: string;
    content: string;
    words: number;
    readingTime: number;
  } = $props();
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
