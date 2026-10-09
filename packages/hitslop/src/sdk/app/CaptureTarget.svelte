<script lang="ts">
  import { tick, type Snippet } from "svelte";
  import type { SlopContext } from "../abi";
  let { ctx, kind, content, assertRenderable }: { ctx: SlopContext; kind: "export" | "icon"; content: Snippet; assertRenderable: () => void } = $props();
  let active = $state(false);
  let failure: { error: unknown } | undefined;
  function register(element: HTMLDivElement) {
    // A body-level surface isolates capture from nested editor layout and clipping.
    window.document.body.append(element);
    const unregister = ctx.capture.registerTarget(kind, {
      element,
      async prepare() {
        active = true;
        await tick();
        assertRenderable();
        if (failure) throw failure.error;
      },
      async restore() { active = false; await tick(); failure = undefined; },
    });
    return { destroy() { unregister(); element.remove(); } };
  }
</script>

<!-- The icon stage is a fixed 512px square with centered art. -->
<div use:register hidden={!active} data-slop-capture-target={kind}
  style:width={kind === "icon" ? "512px" : "100%"}
  style:height={kind === "icon" ? "512px" : undefined}
  style:display={kind === "icon" && active ? "grid" : undefined}
  style:place-items={kind === "icon" ? "center" : undefined}>
  {#if active}
    <svelte:boundary onerror={(error) => { failure = { error }; ctx.reportError(error); }}>
      {@render content()}
    </svelte:boundary>
  {/if}
</div>
