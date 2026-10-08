<script lang="ts">
  // Text edited in place. It renders as text and mounts its textarea only while edited:
  // WebKit form controls are too expensive to mount by the thousand.
  import type { TextHandle } from "../handle-types";
  import { current } from "./context";

  let { handle, text, label, placeholder = "", class: className = "", onenter }: {
    handle: TextHandle;
    /** The field's value from `doc.current`, shown while not editing. */
    text: string;
    label: string;
    placeholder?: string;
    /** Typography and layout; the text and the textarea inherit them. */
    class?: string;
    /** Called for Enter outside IME composition and without Shift. Without it, Enter adds a line. */
    onenter?: () => void;
  } = $props();

  let editing = $state<{ caret: number | null }>();
  /** Places the caret where the person clicked, or at the end. */
  function start(event?: MouseEvent) {
    const target = event?.currentTarget as HTMLElement | undefined;
    const range = event && document.caretRangeFromPoint?.(event.clientX, event.clientY);
    const inText = target && range && target.contains(range.startContainer) && range.startContainer.nodeType === Node.TEXT_NODE;
    editing = { caret: inText ? range.startOffset : null };
  }
  function bind(field: HTMLTextAreaElement, next: TextHandle) {
    return current().bind.text(field, next);
  }
  function focusAt(field: HTMLTextAreaElement, caret: number | null) {
    field.focus();
    const at = Math.min(caret ?? field.value.length, field.value.length);
    field.setSelectionRange(at, at);
  }
  function keydown(event: KeyboardEvent) {
    // An IME's Enter commits its composition; WebKit reports that keydown as keyCode 229.
    if (!onenter || event.key !== "Enter" || event.shiftKey || event.isComposing || event.keyCode === 229) return;
    event.preventDefault();
    onenter();
  }
</script>

<!-- The hidden ::after copy of the text sizes the cell, so the textarea grows with it. -->
<div class="hitslop-editable {className}" data-value={editing ? text : undefined}>
  {#if editing}
    <textarea rows="1" aria-label={label} {placeholder}
      use:bind={handle}
      use:focusAt={editing.caret}
      oninput={(event) => { event.currentTarget.parentElement!.dataset.value = event.currentTarget.value; }}
      onblur={() => { editing = undefined; }}
      onkeydown={keydown}></textarea>
  {:else}
    <div role="textbox" tabindex="0" aria-label={label} aria-multiline="true"
      onfocus={() => start()}
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
  textarea::placeholder {
    color: inherit;
    opacity: 0.55;
  }
</style>
