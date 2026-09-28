<script lang="ts">
  import { getContext, onMount, type Snippet } from "svelte";
  import type { SlopContext } from "../abi";
  import CaptureTarget from "./CaptureTarget.svelte";
  import { slopContext } from "./context";

  let { children, exportView, icon }: {
    children: Snippet;
    exportView?: Snippet;
    icon?: Snippet;
  } = $props();
  const ctx = getContext<SlopContext | undefined>(slopContext);
  if (!ctx) throw new Error("Slop requires a host document; export default defineSlop(App)");
  let renderFailure: { error: unknown } | undefined;
  function assertRenderable() { if (renderFailure) throw renderFailure.error; }
  const describe = (error: unknown) => error instanceof Error ? error.message : String(error);
  function reportRenderError(error: unknown) {
    renderFailure = { error };
    ctx!.reportError(error);
  }
  onMount(() => ctx!.capture.onPrepare(async () => {
    assertRenderable();
    await ctx!.document.flush();
  }));
</script>

<svelte:boundary onerror={reportRenderError}>
  <div data-hitslop-root>
    {@render children()}
  </div>
  {#snippet failed(error)}
    <p role="alert">Could not render this document: {describe(error)}</p>
  {/snippet}
</svelte:boundary>
<!-- Capture snippets fail only their capture, never the editor. -->
{#if exportView}<CaptureTarget {ctx} kind="export" {assertRenderable} content={exportView} />{/if}
{#if icon}<CaptureTarget {ctx} kind="icon" {assertRenderable} content={icon} />{/if}
