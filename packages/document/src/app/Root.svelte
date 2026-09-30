<script lang="ts">
  import { onMount, type Component } from "svelte";
  import CaptureTarget from "./CaptureTarget.svelte";
  import { current } from "./context";

  let { App, Export, Icon }: {
    App: Component;
    Export?: Component<{ mode: "preview" | "export" }>;
    Icon?: Component;
  } = $props();
  let mode = $state<"preview" | "export">("preview");
  const ctx = current();
  let renderFailure: { error: unknown } | undefined;
  function assertRenderable() { if (renderFailure) throw renderFailure.error; }
  const describe = (error: unknown) => error instanceof Error ? error.message : String(error);
  function reportRenderError(error: unknown) {
    renderFailure = { error };
    ctx.reportError(error);
  }
  onMount(() => ctx.capture.onPrepare(async (next) => {
    if (next !== "icon") mode = next;
    assertRenderable();
    await ctx.document.flush();
  }));
</script>

<svelte:boundary onerror={reportRenderError}>
  <div data-hitslop-root>
    <App />
  </div>
  {#snippet failed(error)}
    <p role="alert">Could not render this document: {describe(error)}</p>
  {/snippet}
</svelte:boundary>
<!-- Capture components mount only while capturing; their failures never replace the editor. -->
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
