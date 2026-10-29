<script lang="ts">
  import "@hitslop/tinys/base.css";
  import "./styles.css";
  import { bindText, getDocument } from "@hitslop/tinys/svelte";
  import { tick } from "svelte";
  import { Button, Tabs } from "bits-ui";
  import SvelteMarkdown from "@humanspeak/svelte-markdown";
  import Bold from "@lucide/svelte/icons/bold";
  import Check from "@lucide/svelte/icons/check";
  import Code from "@lucide/svelte/icons/code";
  import Copy from "@lucide/svelte/icons/copy";
  import Heading1 from "@lucide/svelte/icons/heading-1";
  import Heading2 from "@lucide/svelte/icons/heading-2";
  import Italic from "@lucide/svelte/icons/italic";
  import List from "@lucide/svelte/icons/list";
  import Moon from "@lucide/svelte/icons/moon";
  import Quote from "@lucide/svelte/icons/quote";
  import Sun from "@lucide/svelte/icons/sun";
  import schema from "./schema";
  import {
    markdownView, nextMode, prefixLine as prefixedLine, setContent, setMode as writeMode,
    toggleTheme as flipTheme, wrapSelection, type Mode, type Rewrite,
  } from "./markdown";

  const doc = getDocument(schema);

  let textareaEl: HTMLTextAreaElement | undefined = $state();
  let backdropEl: HTMLDivElement | undefined = $state();
  let copied = $state(false);

  const view = $derived(markdownView(doc.current));
  const highlighted = $derived(highlightMarkdown(doc.current.content));

  function isMode(value: string): value is Mode {
    return value === "inplace" || value === "split" || value === "preview";
  }
  function setMode(value: string) {
    if (isMode(value)) void doc.change(tx => writeMode(tx, value));
  }
  function cycleMode() {
    void doc.change(tx => writeMode(tx, nextMode(doc.current.mode)));
    void tick().then(() => textareaEl?.focus());
  }
  function toggleTheme() {
    void doc.change(tx => flipTheme(tx, doc.current));
  }
  function onScroll() {
    if (textareaEl && backdropEl) backdropEl.scrollTop = textareaEl.scrollTop;
  }
  async function apply(rewrite: Rewrite) {
    void doc.change(tx => setContent(tx, rewrite.value));
    await tick();
    textareaEl?.focus();
    textareaEl?.setSelectionRange(rewrite.start, rewrite.end);
  }
  function wrap(marker: string) {
    if (!textareaEl || doc.current.mode === "preview") return;
    void apply(wrapSelection(doc.current.content, textareaEl.selectionStart, textareaEl.selectionEnd, marker));
  }
  function prefixLine(prefix: string) {
    if (!textareaEl || doc.current.mode === "preview") return;
    void apply(prefixedLine(doc.current.content, textareaEl.selectionStart, prefix));
  }
  async function copyMarkdown() {
    try {
      await navigator.clipboard.writeText(doc.current.content);
      copied = true;
      window.setTimeout(() => { copied = false; }, 1800);
    } catch {
      copied = false;
    }
  }
  function onKeydown(event: KeyboardEvent) {
    const mod = event.metaKey || event.ctrlKey;
    if (!mod || event.altKey) return;
    const key = event.key.toLowerCase();
    if (key === "e") {
      event.preventDefault();
      cycleMode();
      return;
    }
    if (event.target !== textareaEl || doc.current.mode === "preview") return;
    if (key === "b") { event.preventDefault(); wrap("**"); }
    else if (key === "i") { event.preventDefault(); wrap("*"); }
    else if (key === "1" && event.shiftKey) { event.preventDefault(); prefixLine("# "); }
    else if (key === "2" && event.shiftKey) { event.preventDefault(); prefixLine("## "); }
  }
  function highlightMarkdown(raw: string): string {
    const escaped = raw.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
    return escaped.split("\n").map((line) => {
      const heading = line.match(/^(#{1,6})(\s+)(.*)$/);
      if (heading) return `<span class="md-syntaxHash">${heading[1]}</span>${heading[2]}<strong class="md-syntaxHeading">${heading[3]}</strong>`;
      const quote = line.match(/^(&gt;)(\s*)(.*)$/);
      if (quote) return `<span class="md-syntaxToken">&gt;</span>${quote[2]}<span class="md-syntaxQuote">${quote[3]}</span>`;
      const list = line.match(/^(\s*(?:[-*+]|\d+\.)\s+)(.*)$/);
      if (list) return `<span class="md-syntaxToken">${list[1]}</span>${formatInline(list[2])}`;
      return formatInline(line);
    }).join("\n");
  }
  function formatInline(text: string): string {
    return text
      .replace(/\*\*([^*]+)\*\*/g, `<span class="md-syntaxToken">**</span><strong class="md-syntaxBold">$1</strong><span class="md-syntaxToken">**</span>`)
      .replace(/(?<!\*)\*([^*]+)\*(?!\*)/g, `<span class="md-syntaxToken">*</span><em class="md-syntaxItalic">$1</em><span class="md-syntaxToken">*</span>`)
      .replace(/`([^`]+)`/g, `<span class="md-syntaxToken">\`</span><code class="md-syntaxCode">$1</code><span class="md-syntaxToken">\`</span>`);
  }
</script>

<svelte:window onkeydown={onKeydown} />
<main
  class={"md-canvas"}
  data-theme={doc.current.theme}
  aria-label="Markdown manuscript"
>
  <article class={"md-sheet"}>
    <header class={"md-header"}>
      <input class={"md-title"} aria-label="Document title" placeholder="Untitled" use:bindText={doc.fields.title} />
      <div class={"md-actions"} data-tiny-export="hide">
        <Tabs.Root value={doc.current.mode} onValueChange={setMode}>
          <Tabs.List class={"md-tabs"} aria-label="Editor modes">
            <Tabs.Trigger value="inplace" class={"md-tab"}>Edit</Tabs.Trigger>
            <Tabs.Trigger value="split" class={"md-tab"}>Split</Tabs.Trigger>
            <Tabs.Trigger value="preview" class={"md-tab"}>Preview</Tabs.Trigger>
          </Tabs.List>
        </Tabs.Root>
        <Button.Root type="button" class={"md-iconBtn"} aria-label={doc.current.theme === "paper" ? "Switch to dark carbon" : "Switch to paper"} onclick={toggleTheme}>
          {#if doc.current.theme === "paper"}<Moon size={14} strokeWidth={1.8} />{:else}<Sun size={14} strokeWidth={1.8} />{/if}
        </Button.Root>
        <Button.Root type="button" class={"md-iconBtn"} data-copied={copied} aria-label="Copy markdown" onclick={copyMarkdown}>
          {#if copied}<Check size={14} strokeWidth={1.8} />{:else}<Copy size={14} strokeWidth={1.8} />{/if}
        </Button.Root>
      </div>
    </header>

    {#if doc.current.mode !== "preview"}
      <section class={"md-toolbar"} data-tiny-export="hide" aria-label="Text formatting">
        <Button.Root type="button" class={"md-toolBtn"} aria-label="Heading 1" onclick={() => prefixLine("# ")}><Heading1 size={14} strokeWidth={1.8} /></Button.Root>
        <Button.Root type="button" class={"md-toolBtn"} aria-label="Heading 2" onclick={() => prefixLine("## ")}><Heading2 size={14} strokeWidth={1.8} /></Button.Root>
        <span class={"md-toolSep"} aria-hidden="true"></span>
        <Button.Root type="button" class={"md-toolBtn"} aria-label="Bold" onclick={() => wrap("**")}><Bold size={14} strokeWidth={1.8} /></Button.Root>
        <Button.Root type="button" class={"md-toolBtn"} aria-label="Italic" onclick={() => wrap("*")}><Italic size={14} strokeWidth={1.8} /></Button.Root>
        <Button.Root type="button" class={"md-toolBtn"} aria-label="Code" onclick={() => wrap("`")}><Code size={14} strokeWidth={1.8} /></Button.Root>
        <span class={"md-toolSep"} aria-hidden="true"></span>
        <Button.Root type="button" class={"md-toolBtn"} aria-label="List item" onclick={() => prefixLine("- ")}><List size={14} strokeWidth={1.8} /></Button.Root>
        <Button.Root type="button" class={"md-toolBtn"} aria-label="Blockquote" onclick={() => prefixLine("> ")}><Quote size={14} strokeWidth={1.8} /></Button.Root>
      </section>
    {/if}

    <section class={"md-body"}>
      {#if doc.current.mode === "inplace"}
        <div class={"md-overtype"}>
          <textarea
            bind:this={textareaEl}
            class={"md-textarea"}
            aria-label="Markdown editor"
            spellcheck="false"
            use:bindText={doc.fields.content}
            onscroll={onScroll}
          ></textarea>
          <div bind:this={backdropEl} class={"md-backdrop"} aria-hidden="true">{@html highlighted + "\n"}</div>
          {#if !doc.current.content}<p class={"md-placeholder"}>Begin the page. Native undo. Real caret.</p>{/if}
        </div>
      {:else if doc.current.mode === "split"}
        <div class={"md-split"}>
          <div class={"md-pane"}>
            <textarea bind:this={textareaEl} class={"md-source"} aria-label="Markdown source" spellcheck="false" use:bindText={doc.fields.content}></textarea>
          </div>
          <div class={"md-divider"} aria-hidden="true"></div>
          <div class={`md-pane md-prose`}>
            {#if doc.current.content.trim()}
              <SvelteMarkdown source={doc.current.content} />
            {:else}
              <p class={"md-empty"}>Nothing on the page yet.</p>
            {/if}
          </div>
        </div>
      {:else}
        <div class={"md-preview"}>
          <article class={"md-prose"}>
            {#if doc.current.content.trim()}
              <SvelteMarkdown source={doc.current.content} />
            {:else}
              <p class={"md-empty"}>Nothing on the page yet. Switch to Edit to begin.</p>
            {/if}
          </article>
        </div>
      {/if}
    </section>

    <footer class={"md-footer"}>
      <span>{view.words} words · {view.chars} chars · {view.readingTime} min read</span>
      <span class={"md-hint"} data-tiny-export="hide">⌘B bold · ⌘I italic · ⌘E views</span>
    </footer>
  </article>

</main>
