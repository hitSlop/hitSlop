<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  import SvelteMarkdown from "@humanspeak/svelte-markdown";
  import type { CaptureProps } from "@hitslop/tinys/svelte";
  import type schema from "./schema";
  import { previewText } from "../_shared/preview";
  import { markdownView } from "./markdown";
  let { snapshot, preview = false }: CaptureProps<typeof schema> & { preview?: boolean } = $props();

  const { content } = $derived(snapshot);
  const view = $derived(markdownView(snapshot));
</script>

<article class={"md-exportSheet"} aria-label="Exported manuscript {view.title}">
  <header>
    <p class={"md-exportEyebrow"}>Manuscript</p>
    <h1 class={"md-exportTitle"}>{view.title}</h1>
    <p class={"md-exportMeta"}>{view.words} words · {view.readingTime} min read</p>
  </header>
  <div class={`md-prose md-exportProse`}>
    {#if !view.blank}
      <SvelteMarkdown source={previewText(content, preview, 1200)} />
    {:else}
      <p class={"md-empty"}>This page is blank.</p>
    {/if}
  </div>
</article>
