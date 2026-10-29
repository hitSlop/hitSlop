<script lang="ts">
  import { onMount, type Component } from "svelte";
  import { current } from "./context";
  import { notice, notify } from "./notice.svelte";

  let { App }: { App: Component } = $props();
  const ctx = current();
  const describe = (error: unknown) => error instanceof Error ? error.message : String(error);
  // The shell offers each unhandled refusal; showing it here keeps it out of the host's
  // issue badge, which is for faults.
  onMount(() => {
    const refused = (event: Event) => {
      event.preventDefault();
      notify((event as CustomEvent<string>).detail);
    };
    document.addEventListener("tiny:refused", refused);
    return () => {
      document.removeEventListener("tiny:refused", refused);
      notice.clear();
    };
  });
</script>

<svelte:boundary onerror={(error) => ctx.reportError(error)}>
  <div data-tiny-root>
    <App />
    <!-- Always present, so screen readers announce what appears in it. -->
    <div data-tiny-notice data-tiny-export="hide" role="status" aria-live="polite">
      {#if notice.current}{#key notice.current.key}<p>{notice.current.text}</p>{/key}{/if}
    </div>
  </div>
  {#snippet failed(error)}
    <p role="alert">Could not render this document: {describe(error)}</p>
  {/snippet}
</svelte:boundary>

<style>
  /* Zero specificity: any selector in the app's CSS restyles the notice. */
  :global(:where([data-tiny-notice])) {
    position: fixed;
    inset-inline: 16px;
    bottom: 16px;
    display: flex;
    justify-content: center;
    pointer-events: none;
    z-index: 2147483000;
  }
  :global(:where([data-tiny-notice] > p)) {
    margin: 0;
    max-width: 36em;
    padding: 8px 14px;
    border-radius: 999px;
    background: rgb(28 28 30 / 0.9);
    color: #fff;
    font: 500 13px/1.35 system-ui, sans-serif;
    text-align: center;
    pointer-events: auto;
  }
</style>
