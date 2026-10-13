<script lang="ts">
  import { onMount, tick, type Component } from "svelte";
  import type { WidgetProps } from "../abi";
  import CaptureTarget from "./CaptureTarget.svelte";
  import { current } from "./context";
  import { notice, notify } from "./notice.svelte";

  let { App, Preview, Export, Icon, Widget }: {
    App: Component;
    Preview?: Component;
    Export?: Component<{ mode: "preview" | "export" }>;
    Icon?: Component;
    Widget?: Component<WidgetProps>;
  } = $props();
  let mode = $state<"preview" | "export">("preview");
  let widget = $state<WidgetProps>();
  const ctx = current();
  const renderer = ctx.capture.isRenderer();
  let editor = $state(!renderer);
  let renderFailure: { error: unknown } | undefined;
  function assertRenderable() { if (renderFailure) throw renderFailure.error; }
  const describe = (error: unknown) => error instanceof Error ? error.message : String(error);
  function reportRenderError(error: unknown) {
    renderFailure = { error };
    ctx.reportError(error);
  }
  onMount(() => ctx.capture.onPrepare(async (next, signal, viewport) => {
    if (next === "widget") widget = viewport;
    if (next === "preview" || next === "export") mode = next;
    if (renderer && ((next === "preview" && !Preview) || (next === "export" && !Export))) {
      editor = true;
      signal.addEventListener("abort", () => { editor = false; renderFailure = undefined; }, { once: true });
      await tick();
      signal.throwIfAborted();
    }
    assertRenderable();
    await ctx.document.flush();
  }));
  // The shell offers each unhandled refusal; showing it here keeps it out of the host's
  // issue badge, which is for faults.
  onMount(() => {
    if (renderer) return;
    const refused = (event: Event) => {
      event.preventDefault();
      notify((event as CustomEvent<string>).detail);
    };
    document.addEventListener("slop:refused", refused);
    return () => {
      document.removeEventListener("slop:refused", refused);
      notice.clear();
    };
  });
</script>

{#if editor}
<svelte:boundary onerror={reportRenderError}>
  <div data-slop-root>
    <App />
    <!-- Always present, so screen readers announce what appears in it. -->
    <div data-slop-notice data-slop-export="hide" role="status" aria-live="polite">
      {#if notice.current}{#key notice.current.key}<p>{notice.current.text}</p>{/key}{/if}
    </div>
  </div>
  {#snippet failed(error)}
    <p role="alert">Could not render this document: {describe(error)}</p>
  {/snippet}
</svelte:boundary>
{/if}
<!-- Capture components mount only while capturing; their failures never replace the editor. -->
{#if Preview}
  <CaptureTarget {ctx} kind="preview" {assertRenderable}>
    {#snippet content()}<Preview />{/snippet}
  </CaptureTarget>
{/if}
{#if Export}
  <CaptureTarget {ctx} kind="export" {assertRenderable}>
    {#snippet content()}<Export {mode} />{/snippet}
  </CaptureTarget>
{/if}
{#if Icon}
  <CaptureTarget {ctx} kind="icon" {assertRenderable}>
    {#snippet content()}<Icon />{/snippet}
  </CaptureTarget>
{/if}

{#if Widget}
  <CaptureTarget {ctx} kind="widget" {assertRenderable}>
    {#snippet content()}{#if widget}<Widget {...widget} />{/if}{/snippet}
  </CaptureTarget>
{/if}

<style>
  /* Zero specificity: any selector in the app's CSS restyles the notice. */
  :global(:where([data-slop-notice])) {
    position: fixed;
    inset-inline: 16px;
    bottom: 16px;
    display: flex;
    justify-content: center;
    pointer-events: none;
    z-index: 2147483000;
  }
  :global(:where([data-slop-notice] > p)) {
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
