<script lang="ts">
  // Text edited in place. It renders as text and mounts its textarea only while edited:
  // WebKit form controls are too expensive to mount by the thousand.
  import { tick } from "svelte";
  import type { TextHandle } from "../handle-types";
  import { bindText } from "./bind-text";

  let { field, label, placeholder = "", class: className = "", onenter }: {
    /** The text field, such as `doc.fields.title` or `doc.at(row).text`. */
    field: TextHandle;
    label: string;
    placeholder?: string;
    /** Typography and layout; the text and the textarea inherit them. */
    class?: string;
    /** Called for Enter outside IME composition and without Shift. Without it, Enter adds a line. */
    onenter?: () => void;
  } = $props();

  const text = $derived(field.value);
  let editing = $state<{ caret: number | null }>();
  let mirror = $state("");
  let display = $state<HTMLDivElement>();
  let composing = false;
  let returningFocus = false;
  /** Places the caret where the person clicked, or at the end. */
  function start(event?: MouseEvent) {
    const target = event?.currentTarget as HTMLElement | undefined;
    const range = event && document.caretRangeFromPoint?.(event.clientX, event.clientY);
    const inText = target && range && target.contains(range.startContainer) && range.startContainer.nodeType === Node.TEXT_NODE;
    editing = { caret: inText ? range.startOffset : null };
  }
  function focusAt(field: HTMLTextAreaElement, caret: number | null) {
    mirror = field.value;
    field.focus();
    const at = Math.min(caret ?? field.value.length, field.value.length);
    field.setSelectionRange(at, at);
  }
  async function finish() {
    // Unmounting lets the host binding drain its pending typing; Escape never reverts it.
    editing = undefined;
    await tick();
    returningFocus = true;
    display?.focus();
    returningFocus = false;
  }
  function keydown(event: KeyboardEvent) {
    // An IME's Enter commits its composition; WebKit reports that keydown as keyCode 229.
    if (composing || event.isComposing || event.keyCode === 229) return;
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      void finish();
      return;
    }
    if (!onenter || event.key !== "Enter" || event.shiftKey) return;
    event.preventDefault();
    onenter();
  }
</script>

<!-- The hidden ::after copy of the text sizes the cell, so the textarea grows with it. -->
<div class="hitslop-editable {className}" data-value={editing ? mirror : undefined}>
  {#if editing}
    <textarea rows="1" aria-label={label} {placeholder}
      use:bindText={field}
      use:focusAt={editing.caret}
      oninput={(event) => { mirror = event.currentTarget.value; }}
      oncompositionstart={() => { composing = true; }}
      oncompositionend={() => { composing = false; }}
      onblur={() => { editing = undefined; }}
      onkeydown={keydown}></textarea>
  {:else}
    <div bind:this={display} role="textbox" tabindex="0" aria-label={label} aria-multiline="true"
      onfocus={() => { if (!returningFocus) start(); }}
      onkeydown={(event) => { if (event.key === "Enter") { event.preventDefault(); start(); } }}
      onmousedown={(event) => { event.preventDefault(); start(event); }}
    >{#if text}{text}{:else}<span data-placeholder>{placeholder}</span>{/if}</div>
  {/if}
</div>

<style>
  .hitslop-editable {
    display: grid;
    min-width: 0;
  }
  .hitslop-editable > *,
  .hitslop-editable[data-value]::after {
    grid-area: 1 / 1;
    min-width: 0;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .hitslop-editable[data-value]::after {
    content: attr(data-value) " ";
    visibility: hidden;
    padding-inline-end: 2px;
  }
  textarea {
    display: block;
    box-sizing: border-box;
    width: 100%;
    height: 100%;
    margin: 0;
    padding: 0;
    border: 0;
    background: transparent;
    resize: none;
    overflow: hidden;
    font: inherit;
    color: inherit;
    letter-spacing: inherit;
    text-decoration: inherit;
    text-transform: inherit;
  }
  div[role="textbox"] {
    cursor: text;
  }
  /* Defaults an author's single class overrides. */
  :where(span[data-placeholder]),
  :where(textarea)::placeholder {
    color: inherit;
    opacity: 0.55;
  }
</style>
